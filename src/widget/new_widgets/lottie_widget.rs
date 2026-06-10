//! LottieWidget — Lottie JSON animation player widget.
//!
//! The LottieWidget parses a Lottie JSON animation, manages play/pause/stop
//! controls, frame rate, loop count, and frame advancement. It emits a signal
//! when the animation finishes.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// LottieWidget — a Lottie JSON animation player widget.
pub struct LottieWidget {
    base: BaseWidget,
    json_data: Option<String>,
    current_frame: u32,
    total_frames: u32,
    playing: bool,
    /// Loop count: 0 = infinite, >0 = number of repetitions.
    loop_count: i32,
    /// Frame rate in frames per second.
    frame_rate: f32,
    /// Internal timer accumulator in milliseconds.
    frame_timer: u64,
    /// Emitted when the animation finishes (all loops completed).
    pub animation_finished: GenericSignal,
}

impl LottieWidget {
    /// Creates a new LottieWidget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LottieWidget, geometry, "LottieWidget"),
            json_data: None,
            current_frame: 0,
            total_frames: 0,
            playing: false,
            loop_count: 0,
            frame_rate: 30.0,
            frame_timer: 0,
            animation_finished: GenericSignal::new(),
        }
    }

    /// Loads Lottie JSON data, parses it, and counts frames.
    /// Returns Ok(()) on success, or an error string if parsing fails.
    pub fn load_json(&mut self, data: &str) -> Result<(), String> {
        if data.is_empty() {
            return Err("JSON data is empty".to_string());
        }

        // Attempt to parse the data as JSON and extract frame-related fields.
        // Lottie JSON has "op" (out point / last frame) and "ip" (in point / first frame).
        let parsed: serde_json::Value =
            serde_json::from_str(data).map_err(|e| format!("Invalid Lottie JSON: {}", e))?;

        let op = parsed
            .get("op")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| "Missing or invalid 'op' field in Lottie JSON".to_string())?;

        let ip = parsed
            .get("ip")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| "Missing or invalid 'ip' field in Lottie JSON".to_string())?;

        let total = (op - ip).max(0.0) as u32;
        if total == 0 {
            return Err("Lottie animation has zero frames".to_string());
        }

        // Extract frame rate if present.
        if let Some(fr) = parsed.get("fr").and_then(|v| v.as_f64()) {
            if fr > 0.0 {
                self.frame_rate = fr as f32;
            }
        }

        self.json_data = Some(data.to_string());
        self.total_frames = total;
        self.current_frame = 0;
        self.frame_timer = 0;
        self.playing = false;
        Ok(())
    }

    /// Starts playback of the animation.
    pub fn play(&mut self) {
        if self.total_frames == 0 {
            return;
        }
        self.playing = true;
        self.base.request_redraw();
    }

    /// Pauses playback, keeping the current frame visible.
    pub fn pause(&mut self) {
        self.playing = false;
        self.base.request_redraw();
    }

    /// Stops playback and resets to the first frame.
    pub fn stop(&mut self) {
        self.playing = false;
        self.current_frame = 0;
        self.frame_timer = 0;
        self.base.request_redraw();
    }

    /// Returns whether the animation is currently playing.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Sets the loop count. 0 = infinite, >0 = number of repetitions.
    pub fn set_loop_count(&mut self, n: i32) {
        self.loop_count = n.max(0);
    }

    /// Returns the current loop count.
    pub fn loop_count(&self) -> i32 {
        self.loop_count
    }

    /// Sets the frame rate in frames per second.
    pub fn set_frame_rate(&mut self, fps: f32) {
        if fps > 0.0 {
            self.frame_rate = fps;
        }
    }

    /// Returns the frame rate.
    pub fn frame_rate(&self) -> f32 {
        self.frame_rate
    }

    /// Returns the current frame index.
    pub fn current_frame(&self) -> u32 {
        self.current_frame
    }

    /// Returns the total number of frames.
    pub fn total_frames(&self) -> u32 {
        self.total_frames
    }

    /// Sets the current frame index directly (clamped to valid range).
    pub fn set_current_frame(&mut self, frame: u32) {
        if self.total_frames == 0 {
            return;
        }
        self.current_frame = frame.min(self.total_frames - 1);
        self.frame_timer = 0;
        self.base.request_redraw();
    }

    /// Advances to the next frame based on the frame timer.
    /// Call this with elapsed milliseconds to drive animation.
    /// Returns true if the frame changed as a result.
    pub fn advance_frame(&mut self) -> bool {
        if self.total_frames == 0 || !self.playing {
            return false;
        }

        let next = self.current_frame + 1;
        if next >= self.total_frames {
            // Reached the end of the sequence.
            if self.loop_count == 0 {
                // Infinite looping: wrap around.
                self.current_frame = 0;
            } else {
                // Finite looping: count down.
                if self.loop_count > 0 {
                    self.loop_count -= 1;
                }
                if self.loop_count == 0 {
                    // All loops completed, stop.
                    self.playing = false;
                    self.animation_finished.emit();
                    return true;
                }
                self.current_frame = 0;
            }
        } else {
            self.current_frame = next;
        }

        self.frame_timer = 0;
        self.base.request_redraw();
        true
    }

    /// Advances the animation timer by the given number of milliseconds.
    /// Returns true if the frame changed as a result.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        if !self.playing || self.total_frames == 0 {
            return false;
        }

        let frame_delay = if self.frame_rate > 0.0 {
            (1000.0 / self.frame_rate as f64) as u64
        } else {
            33
        };

        self.frame_timer += delta_ms;
        if self.frame_timer >= frame_delay {
            self.advance_frame();
            true
        } else {
            false
        }
    }

    /// Returns a reference to the raw JSON data, if loaded.
    pub fn json_data(&self) -> Option<&str> {
        self.json_data.as_deref()
    }
}

impl Widget for LottieWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for LottieWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        if self.total_frames == 0 {
            // Empty state: draw a neutral placeholder.
            context.fill_rounded_rect(rect, 4, Color::rgba(230, 230, 230, 200));
            let font = Font::default();
            let text = "No Lottie animation loaded";
            let metrics = context.measure_text(text, &font);
            let text_x = rect.x + (rect.width as i32 - metrics.width as i32) / 2;
            let text_y = rect.y + rect.height as i32 / 2 + metrics.ascent as i32 / 2;
            context.draw_text(
                Point::new(text_x, text_y),
                text,
                &font,
                Color::rgba(160, 160, 160, 220),
            );
            return;
        }

        // Background.
        let bg = if !is_enabled {
            Color::rgba(200, 200, 200, 100)
        } else {
            Color::rgba(240, 240, 250, 255)
        };
        context.fill_rect(rect, bg);

        // Draw bounding box.
        context.draw_rect_stroke(rect, Color::rgba(100, 100, 180, 150), 1);

        // Draw animation info centered.
        let font = Font::default();
        let info_text =
            format!("Lottie Frame {}/{}", self.current_frame + 1, self.total_frames);
        let metrics = context.measure_text(&info_text, &font);
        let info_x = rect.x + (rect.width as i32 - metrics.width as i32) / 2;
        let info_y = rect.y + rect.height as i32 / 3 + metrics.ascent as i32 / 2;
        context.draw_text(
            Point::new(info_x, info_y),
            &info_text,
            &font,
            Color::rgba(60, 60, 120, 230),
        );

        // Draw play/pause status.
        let status = if self.playing { "▶ Playing" } else { "⏸ Paused" };
        let status_metrics = context.measure_text(status, &font);
        let status_x = rect.x + (rect.width as i32 - status_metrics.width as i32) / 2;
        let status_y = rect.y + rect.height as i32 * 2 / 3 + status_metrics.ascent as i32 / 2;
        let status_color = if self.playing {
            Color::rgba(40, 160, 40, 230)
        } else {
            Color::rgba(180, 100, 40, 230)
        };
        context.draw_text(Point::new(status_x, status_y), status, &font, status_color);

        // Frame counter overlay at top-right.
        let counter_text = format!("{}/{} FPS:{:.0}", self.current_frame + 1, self.total_frames, self.frame_rate);
        let c_metrics = context.measure_text(&counter_text, &font);
        let cx = rect.x + rect.width as i32 - c_metrics.width as i32 - 4;
        let cy = rect.y + 2;
        let pill_w = c_metrics.width as u32 + 8;
        let pill_h = c_metrics.height as u32 + 2;
        let pill_rect = Rect::new(cx - 4, cy - 1, pill_w, pill_h);
        context.fill_rounded_rect(pill_rect, 3, Color::rgba(0, 0, 0, 60));
        context.draw_text(
            Point::new(cx, cy + c_metrics.ascent as i32),
            &counter_text,
            &font,
            Color::WHITE,
        );

        // Progress bar at bottom.
        let progress_bar_height = 6u32;
        let progress_bar_y = rect.y + rect.height as i32 - progress_bar_height as i32 - 4;
        let progress_bar_full = Rect::new(
            rect.x + 4,
            progress_bar_y,
            (rect.width as u32).saturating_sub(8),
            progress_bar_height,
        );
        context.fill_rounded_rect(progress_bar_full, 3, Color::rgba(200, 200, 200, 150));

        if self.total_frames > 0 {
            let fill_ratio = (self.current_frame as f64) / (self.total_frames as f64);
            let filled_width =
                ((progress_bar_full.width as f64) * fill_ratio) as u32;
            if filled_width > 0 {
                let progress_bar_fill = Rect::new(
                    progress_bar_full.x,
                    progress_bar_full.y,
                    filled_width,
                    progress_bar_full.height,
                );
                context.fill_rounded_rect(progress_bar_fill, 3, Color::rgba(60, 120, 220, 200));
            }
        }
    }
}

impl EventHandler for LottieWidget {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button == 1 {
                    if self.geometry().contains_point(*pos) {
                        if self.playing {
                            self.pause();
                        } else {
                            self.play();
                        }
                    }
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
    use crate::core::Point;
    use std::sync::{Arc, Mutex};

    fn make_lottie_json(op: f64, ip: f64, fr: f64) -> String {
        format!(
            r#"{{"op":{},"ip":{},"fr":{},"v":"5.5.2","w":100,"h":100}}"#,
            op, ip, fr
        )
    }

    #[test]
    fn lottie_widget_creation_defaults() {
        let lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        assert_eq!(lottie.total_frames(), 0);
        assert_eq!(lottie.current_frame(), 0);
        assert!(!lottie.is_playing());
        assert_eq!(lottie.loop_count(), 0);
        assert_eq!(lottie.frame_rate(), 30.0);
        assert!(lottie.json_data().is_none());
        assert_eq!(lottie.kind(), WidgetKind::LottieWidget);
    }

    #[test]
    fn lottie_widget_load_json_and_frame_count() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(60.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        assert_eq!(lottie.total_frames(), 60);
        assert_eq!(lottie.current_frame(), 0);
        assert_eq!(lottie.frame_rate(), 30.0);
        assert!(lottie.json_data().is_some());
    }

    #[test]
    fn lottie_widget_empty_data_returns_error() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let result = lottie.load_json("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "JSON data is empty");
    }

    #[test]
    fn lottie_widget_invalid_json_returns_error() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let result = lottie.load_json("not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid Lottie JSON"));
    }

    #[test]
    fn lottie_widget_play_pause_stop() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(30.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();

        assert!(!lottie.is_playing());
        lottie.play();
        assert!(lottie.is_playing());
        lottie.pause();
        assert!(!lottie.is_playing());
        lottie.play();
        assert!(lottie.is_playing());
        lottie.stop();
        assert!(!lottie.is_playing());
        assert_eq!(lottie.current_frame(), 0);
    }

    #[test]
    fn lottie_widget_advance_frame() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(10.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        lottie.play();

        assert_eq!(lottie.current_frame(), 0);
        lottie.advance_frame();
        assert_eq!(lottie.current_frame(), 1);
        lottie.advance_frame();
        assert_eq!(lottie.current_frame(), 2);
    }

    #[test]
    fn lottie_widget_advance_frame_wraps_with_infinite_loop() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(3.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        lottie.set_loop_count(0); // infinite
        lottie.play();

        lottie.advance_frame(); // 0 -> 1
        assert_eq!(lottie.current_frame(), 1);
        lottie.advance_frame(); // 1 -> 2
        assert_eq!(lottie.current_frame(), 2);
        lottie.advance_frame(); // 2 -> wraps to 0
        assert_eq!(lottie.current_frame(), 0);
    }

    #[test]
    fn lottie_widget_animation_finished_signal() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(5.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        lottie.set_loop_count(1);
        lottie.play();

        let finished = Arc::new(Mutex::new(false));
        lottie.animation_finished.connect({
            let finished = Arc::clone(&finished);
            move || {
                *finished.lock().unwrap() = true;
            }
        });

        // Advance through frames 0->4 (5 frames, one loop).
        for _ in 0..4 {
            lottie.advance_frame();
            assert!(!*finished.lock().unwrap());
        }
        // Frame 4 -> wraps: loop_count reaches 0 -> finish.
        lottie.advance_frame();
        assert!(*finished.lock().unwrap());
        assert!(!lottie.is_playing());
    }

    #[test]
    fn lottie_widget_set_frame_rate() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        assert_eq!(lottie.frame_rate(), 30.0);
        lottie.set_frame_rate(60.0);
        assert_eq!(lottie.frame_rate(), 60.0);
        lottie.set_frame_rate(0.0); // should not change
        assert_eq!(lottie.frame_rate(), 60.0);
    }

    #[test]
    fn lottie_widget_set_current_frame() {
        let mut lottie = LottieWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_lottie_json(10.0, 0.0, 30.0);
        lottie.load_json(&json).unwrap();
        lottie.set_current_frame(5);
        assert_eq!(lottie.current_frame(), 5);
        lottie.set_current_frame(999); // clamped
        assert_eq!(lottie.current_frame(), 9);
    }
}
