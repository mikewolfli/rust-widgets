// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SignaturePad widget — a touch-friendly freehand drawing surface.
//!
//! Captures pen strokes as a list of [`SignatureStroke`]s, each a polyline of
//! [`Point`]s. A stroke is in progress between a press and the matching release,
//! and every move appends a point. Optional smoothing collapses near-duplicate
//! points so a jittery finger produces a clean line rather than a cloud of
//! specks. The pad supports undo, clear, and export to a plain point list.

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_f64, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A single continuous pen stroke, stored as a polyline of points.
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureStroke {
    points: Vec<Point>,
}

impl SignatureStroke {
    /// Creates an empty stroke.
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    /// Creates a stroke from a list of points.
    pub fn from_points(points: Vec<Point>) -> Self {
        Self { points }
    }

    /// Appends a point to the stroke.
    pub fn push(&mut self, point: Point) {
        self.points.push(point);
    }

    /// Returns the stroke's points.
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    /// Returns the number of points in the stroke.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns true when the stroke has no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

impl Default for SignatureStroke {
    fn default() -> Self {
        Self::new()
    }
}

/// A touch-friendly signature capture surface.
///
/// A drag (press → move → release) records one stroke on top of the previous
/// ones. The last stroke can be undone, or the whole pad cleared. `changed`
/// fires whenever the stroke set changes, and `stroke_count` reflects the
/// current total.
pub struct SignaturePad {
    base: BaseWidget,
    strokes: Vec<SignatureStroke>,
    current: Option<SignatureStroke>,
    stroke_color: Color,
    stroke_width: u32,
    /// Minimum distance, in pixels, a new point must be from the previous one
    /// before it is recorded. Points closer than this are dropped.
    min_point_distance: f32,
    /// Emitted with no payload whenever the committed stroke set changes.
    pub changed: GenericSignal,
    /// Emitted with the final stroke when a drag is released and committed.
    pub stroke_completed: Signal1<SignatureStroke>,
}

impl SignaturePad {
    /// Creates an empty signature pad.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SignaturePad, geometry, "SignaturePad"),
            strokes: Vec::new(),
            current: None,
            stroke_color: Color::rgb(20, 20, 20),
            stroke_width: 2,
            min_point_distance: 1.5,
            changed: GenericSignal::new(),
            stroke_completed: Signal1::new(),
        }
    }

    /// Returns the committed strokes (excluding any in-progress stroke).
    pub fn strokes(&self) -> &[SignatureStroke] {
        &self.strokes
    }

    /// Returns the total number of committed strokes.
    pub fn stroke_count(&self) -> usize {
        self.strokes.len()
    }

    /// Returns true when the pad has no committed strokes.
    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty()
    }

    /// Removes the most recently committed stroke, returning whether one was
    /// removed. Emits `changed` when a stroke is actually removed.
    pub fn undo(&mut self) -> bool {
        if self.strokes.pop().is_some() {
            self.changed.emit();
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Removes every committed stroke. Emits `changed`.
    pub fn clear(&mut self) {
        if !self.strokes.is_empty() {
            self.strokes.clear();
            self.current = None;
            self.changed.emit();
            self.base.request_redraw();
        }
    }

    /// Returns the pen stroke color.
    pub fn stroke_color(&self) -> Color {
        self.stroke_color
    }

    /// Sets the pen stroke color and requests a redraw.
    pub fn set_stroke_color(&mut self, color: Color) {
        self.stroke_color = color;
        self.base.request_redraw();
    }

    /// Returns the pen stroke width in pixels.
    pub fn stroke_width(&self) -> u32 {
        self.stroke_width
    }

    /// Sets the pen stroke width, floored at 1, and requests a redraw.
    pub fn set_stroke_width(&mut self, width: u32) {
        self.stroke_width = width.max(1);
        self.base.request_redraw();
    }

    /// Returns the minimum distance between recorded points, in pixels.
    pub fn min_point_distance(&self) -> f32 {
        self.min_point_distance
    }

    /// Sets the minimum distance between recorded points. Values below 0 collapse
    /// to 0 (every point is recorded).
    pub fn set_min_point_distance(&mut self, distance: f32) {
        self.min_point_distance = distance.max(0.0);
    }

    /// Exports every committed stroke's points as a flat list, each stroke
    /// followed by a `(-1, -1)` sentinel so the caller can reconstruct stroke
    /// boundaries from a single vector.
    pub fn export_polylines(&self) -> Vec<Point> {
        let mut out = Vec::new();
        for stroke in &self.strokes {
            out.extend_from_slice(stroke.points());
            out.push(Point::new(-1, -1));
        }
        out
    }

    fn in_progress(&self) -> bool {
        self.current.is_some()
    }

    fn begin_stroke(&mut self, point: Point) {
        let mut stroke = SignatureStroke::new();
        stroke.push(point);
        self.current = Some(stroke);
    }

    fn extend_stroke(&mut self, point: Point) {
        let Some(current) = self.current.as_mut() else {
            return;
        };
        // Drop points closer than the configured threshold (smoothing).
        if let Some(last) = current.points().last() {
            let dx = (point.x - last.x) as f32;
            let dy = (point.y - last.y) as f32;
            if (dx * dx + dy * dy).sqrt() < self.min_point_distance {
                return;
            }
        }
        current.push(point);
        self.base.request_redraw();
    }

    fn end_stroke(&mut self) {
        let Some(stroke) = self.current.take() else {
            return;
        };
        // A single-point stroke still counts: it is a tap, which for a signature
        // pad is a legitimate dot.
        self.strokes.push(stroke.clone());
        self.stroke_completed.emit(stroke);
        self.changed.emit();
        self.base.request_redraw();
    }
}

impl Widget for SignaturePad {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(320, 160)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `SignaturePad`'s property contract.
impl WidgetProperties for SignaturePad {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "stroke_count" => Ok(CapabilityValue::UInt(self.stroke_count() as u64)),
            "stroke_width" => Ok(CapabilityValue::UInt(self.stroke_width() as u64)),
            "stroke_color" => Ok(CapabilityValue::Color(self.stroke_color())),
            "min_point_distance" => Ok(CapabilityValue::Float(self.min_point_distance() as f64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "stroke_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "stroke_width" => {
                self.set_stroke_width(expect_usize(value)? as u32);
                Ok(())
            }
            "stroke_color" => match value {
                CapabilityValue::Color(color) => {
                    self.set_stroke_color(color);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "min_point_distance" => {
                self.set_min_point_distance(expect_f64(value)? as f32);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "stroke_count",
            "stroke_width",
            "stroke_color",
            "min_point_distance",
            BASE_PROPERTY_NAMES
        ]
    }

    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "undo" => {
                let _ = self.undo();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for SignaturePad {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        let rect = self.geometry();
        match event {
            Event::MousePress { pos, button: 1 } if rect.contains_point(*pos) => {
                self.begin_stroke(*pos);
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } if rect.contains_point(*pos) => {
                self.begin_stroke(*pos);
            }
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } if self.in_progress() => {
                self.extend_stroke(*pos);
            }
            #[cfg(feature = "touch")]
            Event::TouchMove { pos, .. } if self.in_progress() => {
                self.extend_stroke(*pos);
            }
            Event::MouseRelease { .. } if self.in_progress() => {
                self.end_stroke();
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } if self.in_progress() => {
                self.end_stroke();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for SignaturePad {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // The pad surface, its frame, and the empty-state baseline are chrome, so
        // they resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Without the theme step the pad
        // stayed white in both appearances and a switch changed nothing.
        //
        // `self.stroke_color` is deliberately *not* themed: it is the caller's
        // configured ink, which the `stroke_color` property writes, so the caller's
        // value must win over anything the theme says.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::theme::resolved_theme_style("signature_pad");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| background.blend(&Color::BLACK, 0.18));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The hint line is secondary chrome: derived from the resolved colours so it
        // stays visible against either surface.
        let hint = background.blend(&text_color, 0.4);

        context.fill_rect(rect, background);
        context.draw_rect(rect, border);

        // Draw committed strokes.
        for stroke in &self.strokes {
            draw_stroke(context, stroke, self.stroke_color, self.stroke_width);
        }
        // Draw the in-progress stroke on top.
        if let Some(current) = &self.current {
            draw_stroke(context, current, self.stroke_color, self.stroke_width);
        }

        // Empty-state hint: a baseline that reads like a signature line.
        if self.strokes.is_empty() && self.current.is_none() {
            let mid_y = rect.y + rect.height as i32 / 2;
            let inset = rect.width as i32 / 6;
            context.draw_line_stroke(
                Point::new(rect.x + inset, mid_y),
                Point::new(rect.x + rect.width as i32 - inset, mid_y),
                hint,
                1,
            );
        }
    }
}

/// Draws a single stroke as a connected polyline.
fn draw_stroke(context: &mut RenderContext, stroke: &SignatureStroke, color: Color, width: u32) {
    let points = stroke.points();
    match points.len() {
        0 => {}
        1 => {
            // A single point renders as a dot.
            context.draw_line_stroke_aa(points[0], points[0], color, width.max(2));
        }
        _ => {
            for pair in points.windows(2) {
                context.draw_line_stroke_aa(pair[0], pair[1], color, width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn pad() -> SignaturePad {
        SignaturePad::new(Rect::new(0, 0, 320, 160))
    }

    #[test]
    fn new_pad_is_empty() {
        let p = pad();
        assert!(p.is_empty());
        assert_eq!(p.stroke_count(), 0);
        assert!(p.strokes().is_empty());
    }

    #[test]
    fn a_press_move_release_records_one_stroke() {
        let mut p = pad();
        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_move(30, 40));
        p.handle_event(&Event::mouse_move(60, 50));
        p.handle_event(&Event::mouse_release(60, 50, 1));

        assert_eq!(p.stroke_count(), 1);
        assert!(!p.is_empty());
        assert_eq!(p.strokes()[0].len(), 3);
        assert_eq!(p.strokes()[0].points()[0], Point::new(10, 20));
    }

    #[test]
    fn smoothing_drops_near_duplicate_points() {
        let mut p = pad();
        p.handle_event(&Event::mouse_press(10, 20, 1));
        // Two near-identical points: both within the 1.5px threshold of the
        // previous point, so neither is recorded.
        p.handle_event(&Event::mouse_move(10, 20));
        p.handle_event(&Event::mouse_move(10, 21));
        p.handle_event(&Event::mouse_release(10, 21, 1));

        let stroke = &p.strokes()[0];
        // Only the starting press survived; the near-duplicate moves were dropped.
        assert_eq!(stroke.len(), 1);
        assert_eq!(stroke.points()[0], Point::new(10, 20));
    }

    #[test]
    fn undo_and_clear_work() {
        let mut p = pad();
        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_release(10, 20, 1));
        assert_eq!(p.stroke_count(), 1);

        assert!(p.undo());
        assert_eq!(p.stroke_count(), 0);
        assert!(!p.undo()); // nothing left to undo

        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_release(10, 20, 1));
        assert_eq!(p.stroke_count(), 1);
        p.clear();
        assert!(p.is_empty());
    }

    #[test]
    fn changed_and_stroke_completed_signals_fire() {
        let mut p = pad();
        let changed = Arc::new(AtomicUsize::new(0));
        let c = changed.clone();
        p.changed.connect(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        let completed = Arc::new(AtomicUsize::new(0));
        let cc = completed.clone();
        p.stroke_completed.connect(move |_| {
            cc.fetch_add(1, Ordering::SeqCst);
        });

        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_release(10, 20, 1));
        assert_eq!(changed.load(Ordering::SeqCst), 1);
        assert_eq!(completed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn export_polylines_emits_sentinels() {
        let mut p = pad();
        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_move(30, 40));
        p.handle_event(&Event::mouse_release(30, 40, 1));
        p.handle_event(&Event::mouse_press(50, 60, 1));
        p.handle_event(&Event::mouse_release(50, 60, 1));

        let flat = p.export_polylines();
        // Stroke 1 (2 points) + sentinel + stroke 2 (1 point) + sentinel.
        assert_eq!(flat.len(), 2 + 1 + 1 + 1);
        assert_eq!(flat[2], Point::new(-1, -1));
        assert_eq!(flat[4], Point::new(-1, -1));
    }

    #[test]
    #[cfg(feature = "touch")]
    fn touch_events_record_strokes_too() {
        let mut p = pad();
        p.handle_event(&crate::event::Event::touch_begin(5, 5, 1));
        p.handle_event(&crate::event::Event::touch_move(40, 40, 1));
        p.handle_event(&crate::event::Event::touch_end(40, 40, 1));
        assert_eq!(p.stroke_count(), 1);
    }

    #[test]
    fn stroke_width_is_floored_at_one() {
        let mut p = pad();
        p.set_stroke_width(0);
        assert_eq!(p.stroke_width(), 1);
    }

    #[test]
    fn disabled_pad_ignores_input() {
        let mut p = pad();
        p.set_enabled(false);
        p.handle_event(&Event::mouse_press(10, 20, 1));
        p.handle_event(&Event::mouse_release(10, 20, 1));
        assert!(p.is_empty());
    }
}
