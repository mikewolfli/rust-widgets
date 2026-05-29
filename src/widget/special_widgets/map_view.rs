//! MapView widget.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// One map marker with logical world coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct MapMarker {
    /// Stable marker id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// World x coordinate in logical map units.
    pub x: f32,
    /// World y coordinate in logical map units.
    pub y: f32,
}

impl MapMarker {
    /// Creates a map marker.
    pub fn new(id: impl Into<String>, label: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            x,
            y,
        }
    }
}

/// Lightweight map view with pan/zoom and marker selection.
pub struct MapView {
    base: BaseWidget,
    center_x: f32,
    center_y: f32,
    zoom: f32,
    markers: Vec<MapMarker>,
    selected_marker: Option<usize>,
    /// Emitted when center changes. Payload is (x, y).
    pub center_changed: Signal1<(f32, f32)>,
    /// Emitted when zoom changes.
    pub zoom_changed: Signal1<f32>,
    /// Emitted when marker is selected. Payload is marker id.
    pub marker_selected: Signal1<String>,
}

impl MapView {
    /// Creates an empty map view.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Canvas, geometry, "MapView"),
            center_x: 0.0,
            center_y: 0.0,
            zoom: 1.0,
            markers: Vec::new(),
            selected_marker: None,
            center_changed: Signal1::new(),
            zoom_changed: Signal1::new(),
            marker_selected: Signal1::new(),
        }
    }

    /// Returns map center.
    pub fn center(&self) -> (f32, f32) {
        (self.center_x, self.center_y)
    }

    /// Sets map center.
    pub fn set_center(&mut self, x: f32, y: f32) {
        if (self.center_x - x).abs() < f32::EPSILON && (self.center_y - y).abs() < f32::EPSILON {
            return;
        }
        self.center_x = x;
        self.center_y = y;
        self.center_changed.emit((x, y));
        self.base.request_redraw();
    }

    /// Pans map center by world delta.
    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.set_center(self.center_x + dx, self.center_y + dy);
    }

    /// Returns zoom level.
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Sets zoom level.
    pub fn set_zoom(&mut self, zoom: f32) {
        let next = zoom.clamp(0.25, 8.0);
        if (self.zoom - next).abs() < f32::EPSILON {
            return;
        }
        self.zoom = next;
        self.zoom_changed.emit(next);
        self.base.request_redraw();
    }

    /// Zooms around current center.
    pub fn zoom_by(&mut self, factor: f32) {
        if factor <= 0.0 {
            return;
        }
        self.set_zoom(self.zoom * factor);
    }

    /// Replaces map markers.
    pub fn set_markers(&mut self, markers: Vec<MapMarker>) {
        self.markers = markers;
        if let Some(index) = self.selected_marker {
            if index >= self.markers.len() {
                self.selected_marker = None;
            }
        }
        self.base.request_redraw();
    }

    /// Returns marker list.
    pub fn markers(&self) -> &[MapMarker] {
        &self.markers
    }

    /// Returns selected marker id.
    pub fn selected_marker_id(&self) -> Option<&str> {
        let index = self.selected_marker?;
        self.markers.get(index).map(|marker| marker.id.as_str())
    }

    fn world_to_screen(&self, world_x: f32, world_y: f32) -> (f32, f32) {
        let rect = self.geometry();
        let sx = rect.x as f32 + rect.width as f32 / 2.0 + (world_x - self.center_x) * self.zoom;
        let sy = rect.y as f32 + rect.height as f32 / 2.0 + (world_y - self.center_y) * self.zoom;
        (sx, sy)
    }

    fn hit_marker_index(&self, pos: Point) -> Option<usize> {
        let px = pos.x as f32;
        let py = pos.y as f32;
        for (index, marker) in self.markers.iter().enumerate() {
            let (sx, sy) = self.world_to_screen(marker.x, marker.y);
            let dx = sx - px;
            let dy = sy - py;
            if dx * dx + dy * dy <= 64.0 {
                return Some(index);
            }
        }
        None
    }

    fn select_marker(&mut self, index: usize) -> bool {
        if index >= self.markers.len() {
            return false;
        }
        if self.selected_marker == Some(index) {
            return true;
        }
        self.selected_marker = Some(index);
        if let Some(marker) = self.markers.get(index) {
            self.marker_selected.emit(marker.id.clone());
        }
        self.base.request_redraw();
        true
    }
}

impl Widget for MapView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for MapView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::Wheel { delta, .. } => {
                if delta.y < 0 {
                    self.zoom_by(1.15);
                } else if delta.y > 0 {
                    self.zoom_by(0.87);
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                37 => self.pan_by(-20.0 / self.zoom, 0.0),
                38 => self.pan_by(0.0, -20.0 / self.zoom),
                39 => self.pan_by(20.0 / self.zoom, 0.0),
                40 => self.pan_by(0.0, 20.0 / self.zoom),
                187 | 107 => self.zoom_by(1.12),
                189 | 109 => self.zoom_by(0.9),
                _ => {}
            },
            Event::MousePress { pos, button: 1 } => {
                if let Some(index) = self.hit_marker_index(*pos) {
                    let _ = self.select_marker(index);
                }
            }
            _ => {}
        }
    }
}

impl Draw for MapView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(235, 243, 250));
        context.draw_rect(rect, Color::from_rgb(168, 184, 201));

        // Draw coarse map grid for pan/zoom visual feedback.
        let step = (40.0 * self.zoom.clamp(0.5, 2.0)) as i32;
        if step > 8 {
            let mut x = rect.x;
            while x < rect.x + rect.width as i32 {
                context.draw_line(
                    Point::new(x, rect.y),
                    Point::new(x, rect.y + rect.height as i32),
                    Color::from_rgb(210, 220, 233),
                );
                x += step;
            }

            let mut y = rect.y;
            while y < rect.y + rect.height as i32 {
                context.draw_line(
                    Point::new(rect.x, y),
                    Point::new(rect.x + rect.width as i32, y),
                    Color::from_rgb(210, 220, 233),
                );
                y += step;
            }
        }

        for (index, marker) in self.markers.iter().enumerate() {
            let (sx, sy) = self.world_to_screen(marker.x, marker.y);
            let marker_rect = Rect::new((sx as i32) - 3, (sy as i32) - 3, 6, 6);
            let color = if self.selected_marker == Some(index) {
                Color::from_rgb(230, 88, 76)
            } else {
                Color::from_rgb(59, 114, 190)
            };
            context.fill_rect(marker_rect, color);
            context.draw_text(
                Point::new((sx as i32) + 6, sy as i32),
                &marker.label,
                &Font::default(),
                Color::from_rgb(33, 43, 56),
            );
        }

        context.draw_text(
            Point::new(rect.x + 8, rect.y + 16),
            &format!(
                "Center ({:.1}, {:.1})  Zoom {:.2}x",
                self.center_x, self.center_y, self.zoom
            ),
            &Font::default(),
            Color::from_rgb(45, 58, 74),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn pan_and_zoom_update_state() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        map.pan_by(15.0, -8.0);
        map.zoom_by(1.5);

        let (cx, cy) = map.center();
        assert!((cx - 15.0).abs() < 0.001);
        assert!((cy + 8.0).abs() < 0.001);
        assert!((map.zoom() - 1.5).abs() < 0.001);
    }

    #[test]
    fn wheel_event_changes_zoom() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        let before = map.zoom();
        map.handle_event(&Event::wheel(0, -120, 0));
        let after_in = map.zoom();
        assert!(after_in > before);

        map.handle_event(&Event::wheel(0, 120, 0));
        let after_out = map.zoom();
        assert!(after_out < after_in);
    }

    #[test]
    fn marker_selection_emits_signal() {
        let mut map = MapView::new(Rect::new(0, 0, 400, 240));
        map.set_markers(vec![MapMarker::new("m1", "Alpha", 0.0, 0.0)]);

        let selected = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = selected.clone();
        map.marker_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        map.handle_event(&Event::mouse_press(200, 120, 1));

        assert_eq!(map.selected_marker_id(), Some("m1"));
        let got = selected
            .lock()
            .ok()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        assert_eq!(got, vec!["m1".to_string()]);
    }
}
