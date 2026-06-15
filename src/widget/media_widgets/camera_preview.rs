//! CameraPreview widget — camera viewfinder preview area.
//!
//! Provides a visual placeholder for camera preview with controls overlay,
//! zoom support, mirror mode, and click-to-toggle behaviour.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Camera preview widget — displays a camera viewfinder area with controls.
///
/// Renders a camera icon placeholder and optional control overlay buttons.
/// Supports zoom, mirror/flip, and preview start/stop state management.
pub struct CameraPreview {
    base: BaseWidget,
    /// Whether the camera preview is currently active.
    is_active: bool,
    /// Camera resolution as (width, height).
    resolution: (u32, u32),
    /// Camera device identifier.
    camera_id: u32,
    /// Whether the preview is mirrored (horizontally flipped).
    mirror_mode: bool,
    /// Whether control buttons are shown overlaid on the preview.
    show_controls: bool,
    /// Current zoom level (1.0 = normal).
    zoom_level: f32,
}

impl CameraPreview {
    /// Creates a new CameraPreview widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CameraPreview, geometry, "CameraPreview"),
            is_active: false,
            resolution: (640, 480),
            camera_id: 0,
            mirror_mode: false,
            show_controls: true,
            zoom_level: 1.0,
        }
    }

    /// Starts the camera preview.
    pub fn start_preview(&mut self) {
        self.is_active = true;
        self.base.request_redraw();
    }

    /// Stops the camera preview.
    pub fn stop_preview(&mut self) {
        self.is_active = false;
        self.base.request_redraw();
    }

    /// Returns whether the camera preview is active.
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Toggles the preview on/off.
    pub fn toggle_preview(&mut self) {
        if self.is_active {
            self.stop_preview();
        } else {
            self.start_preview();
        }
    }

    /// Sets the camera device ID.
    pub fn set_camera_id(&mut self, id: u32) {
        self.camera_id = id;
    }

    /// Returns the current camera device ID.
    pub fn camera_id(&self) -> u32 {
        self.camera_id
    }

    /// Sets the camera capture resolution.
    pub fn set_resolution(&mut self, w: u32, h: u32) {
        self.resolution = (w.max(1), h.max(1));
        self.base.request_redraw();
    }

    /// Returns the current camera resolution.
    pub fn resolution(&self) -> (u32, u32) {
        self.resolution
    }

    /// Enables or disables mirror mode (horizontal flip).
    pub fn set_mirror_mode(&mut self, mirrored: bool) {
        self.mirror_mode = mirrored;
        self.base.request_redraw();
    }

    /// Returns whether mirror mode is enabled.
    pub fn is_mirror_mode(&self) -> bool {
        self.mirror_mode
    }

    /// Sets the zoom level (1.0 = normal, 2.0 = 2x, etc.).
    pub fn set_zoom(&mut self, level: f32) {
        self.zoom_level = level.clamp(1.0, 10.0);
        self.base.request_redraw();
    }

    /// Returns the current zoom level.
    pub fn zoom_level(&self) -> f32 {
        self.zoom_level
    }

    /// Zooms in by one step.
    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom_level + 0.5);
    }

    /// Zooms out by one step.
    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom_level - 0.5);
    }

    /// Makes the control overlay visible.
    pub fn show_controls(&mut self) {
        self.show_controls = true;
        self.base.request_redraw();
    }

    /// Hides the control overlay.
    pub fn hide_controls(&mut self) {
        self.show_controls = false;
        self.base.request_redraw();
    }

    /// Returns whether controls are currently visible.
    pub fn controls_visible(&self) -> bool {
        self.show_controls
    }
}

impl Widget for CameraPreview {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for CameraPreview {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let w = rect.width as i32;
        let h = rect.height as i32;

        if w <= 0 || h <= 0 {
            return;
        }

        // Create default fonts for text rendering
        let small_font = Font::new("sans-serif", 11.0, false, false);
        let normal_font = Font::new("sans-serif", 13.0, false, false);

        if self.is_active {
            // Draw active camera view — dark viewfinder area
            let viewfinder_color = Color::rgba(30, 30, 40, 255);
            context.fill_rect(rect, viewfinder_color);

            // Draw resolution info text
            let res_text = format!("{}x{}", self.resolution.0, self.resolution.1);
            context.draw_text(
                Point::new(rect.x + 6, rect.y + 14),
                &res_text,
                &small_font,
                Color::rgba(200, 200, 200, 200),
                HorizontalAlignment::Left,
            );

            // Draw camera ID
            let id_text = format!("Camera #{}", self.camera_id);
            context.draw_text(
                Point::new(rect.x + 6, rect.y + h - 10),
                &id_text,
                &small_font,
                Color::rgba(200, 200, 200, 200),
                HorizontalAlignment::Left,
            );

            // Draw zoom level indicator
            let zoom_text = format!("{:.1}x", self.zoom_level);
            context.draw_text(
                Point::new(rect.x + w - 40, rect.y + 14),
                &zoom_text,
                &normal_font,
                Color::rgba(255, 255, 255, 220),
                HorizontalAlignment::Left,
            );

            // Draw mirror indicator
            if self.mirror_mode {
                context.draw_text(
                    Point::new(rect.x + w / 2 - 20, rect.y + h - 10),
                    "MIRROR",
                    &small_font,
                    Color::rgba(100, 200, 255, 200),
                    HorizontalAlignment::Left,
                );
            }

            // Draw a subtle crosshair
            let cx = rect.x + w / 2;
            let cy = rect.y + h / 2;
            let crosshair_color = Color::rgba(100, 100, 100, 100);
            context.draw_line(Point::new(cx, cy - 15), Point::new(cx, cy + 15), crosshair_color);
            context.draw_line(Point::new(cx - 15, cy), Point::new(cx + 15, cy), crosshair_color);

            // Draw border indicating active preview
            let border_color = Color::rgba(0, 200, 50, 200);
            context.draw_rect_stroke(rect, border_color, 2);

            // Draw control overlay if visible
            if self.show_controls {
                // Top-right corner controls background
                let control_bg = Rect::new(rect.x + w - 44, rect.y + 4, 40, 80);
                context.fill_rect(control_bg, Color::rgba(0, 0, 0, 120));

                // Draw zoom +/- buttons as simple colored rects
                let zoom_in_btn = Rect::new(rect.x + w - 42, rect.y + 6, 36, 36);
                context.fill_rect(zoom_in_btn, Color::rgba(255, 255, 255, 80));

                let zoom_out_btn = Rect::new(rect.x + w - 42, rect.y + 46, 36, 36);
                context.fill_rect(zoom_out_btn, Color::rgba(255, 255, 255, 80));
            }

            // Draw a green "recording" dot indicator
            let dot_size = 8;
            let dot_rect = Rect::new(rect.x + 6, rect.y + 6, dot_size, dot_size);
            context.fill_rect(dot_rect, Color::rgba(0, 255, 0, 255));
        } else {
            // Draw inactive camera view — dark gray with camera icon placeholder
            context.fill_rect(rect, Color::rgba(50, 50, 60, 255));

            // Draw a simple camera icon placeholder (rounded rectangle shape)
            let icon_w = 48u32;
            let icon_h = 36u32;
            let icon_x = rect.x + (w - icon_w as i32) / 2;
            let icon_y = rect.y + (h - icon_h as i32) / 2 - 10;
            let icon_rect = Rect::new(icon_x, icon_y, icon_w, icon_h);
            context.fill_rounded_rect(icon_rect, 6, Color::rgba(100, 100, 120, 200));

            // Lens circle
            let lens_center_x = icon_x + icon_w as i32 / 2;
            let lens_center_y = icon_y + icon_h as i32 / 2;
            let lens_rect = Rect::new(lens_center_x - 8, lens_center_y - 8, 16, 16);
            context.fill_rounded_rect(lens_rect, 8, Color::rgba(70, 70, 90, 255));

            // Flash dot
            let flash_rect = Rect::new(icon_x + icon_w as i32 - 10, icon_y + 4, 6, 6);
            context.fill_rounded_rect(flash_rect, 3, Color::rgba(200, 200, 200, 150));

            // "Camera Off" label
            context.draw_text(
                Point::new(rect.x + w / 2 - 30, rect.y + h / 2 + 20),
                "Camera Off",
                &normal_font,
                Color::rgba(150, 150, 160, 200),
                HorizontalAlignment::Left,
            );

            // Border
            context.draw_rect_stroke(rect, Color::rgba(100, 100, 110, 200), 1);
        }
    }
}

impl EventHandler for CameraPreview {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos: _, button } => {
                if *button == 1 {
                    // Left-click toggles preview
                    self.toggle_preview();
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_preview_default_state() {
        let cp = CameraPreview::new(Rect::new(0, 0, 320, 240));
        assert!(!cp.is_active());
        assert_eq!(cp.camera_id(), 0);
        assert_eq!(cp.resolution(), (640, 480));
        assert!(!cp.is_mirror_mode());
        assert!(cp.controls_visible());
        assert!((cp.zoom_level() - 1.0).abs() < f32::EPSILON);
        assert_eq!(cp.kind(), WidgetKind::CameraPreview);
    }

    #[test]
    fn camera_preview_toggle() {
        let mut cp = CameraPreview::new(Rect::new(0, 0, 320, 240));
        assert!(!cp.is_active());
        cp.start_preview();
        assert!(cp.is_active());
        cp.stop_preview();
        assert!(!cp.is_active());
        cp.toggle_preview();
        assert!(cp.is_active());
        cp.toggle_preview();
        assert!(!cp.is_active());
    }

    #[test]
    fn camera_preview_zoom() {
        let mut cp = CameraPreview::new(Rect::new(0, 0, 320, 240));
        assert!((cp.zoom_level() - 1.0).abs() < f32::EPSILON);
        cp.zoom_in();
        assert!((cp.zoom_level() - 1.5).abs() < f32::EPSILON);
        cp.zoom_in();
        assert!((cp.zoom_level() - 2.0).abs() < f32::EPSILON);
        cp.zoom_out();
        assert!((cp.zoom_level() - 1.5).abs() < f32::EPSILON);
        cp.set_zoom(5.0);
        assert!((cp.zoom_level() - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn camera_preview_camera_id_and_resolution() {
        let mut cp = CameraPreview::new(Rect::new(0, 0, 320, 240));
        cp.set_camera_id(2);
        assert_eq!(cp.camera_id(), 2);
        cp.set_resolution(1920, 1080);
        assert_eq!(cp.resolution(), (1920, 1080));
    }

    #[test]
    fn camera_preview_mirror_and_controls() {
        let mut cp = CameraPreview::new(Rect::new(0, 0, 320, 240));
        assert!(!cp.is_mirror_mode());
        cp.set_mirror_mode(true);
        assert!(cp.is_mirror_mode());
        assert!(cp.controls_visible());
        cp.hide_controls();
        assert!(!cp.controls_visible());
        cp.show_controls();
        assert!(cp.controls_visible());
    }

    #[test]
    fn camera_preview_click_toggles() {
        let mut cp = CameraPreview::new(Rect::new(0, 0, 320, 240));
        assert!(!cp.is_active());
        cp.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(cp.is_active());
        cp.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(!cp.is_active());
    }
}
