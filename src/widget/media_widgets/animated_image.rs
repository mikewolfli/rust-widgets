//! AnimatedImage widget — plays animated images (GIF/APNG/WebP frame sequences).
//!
//! The AnimatedImage widget manages a sequence of frames with individual delays,
//! supports play/pause/stop controls, loop count configuration, and emits signals
//! when animation finishes or the current frame changes.

use crate::core::{HorizontalAlignment, Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A single frame of animation data.
pub struct AnimatedFrame {
    /// Raw RGBA pixel data for this frame (width * height * 4 bytes).
    pub data: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Display duration for this frame in milliseconds.
    pub delay_ms: u64,
}

/// Image encoding format hint used when parsing animated image data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatedImageFormat {
    /// Graphics Interchange Format.
    Gif,
    /// Animated PNG.
    Apng,
    /// Animated WebP.
    WebP,
}

/// AnimatedImage widget for playing animated image sequences.
pub struct AnimatedImage {
    base: BaseWidget,
    frames: Vec<AnimatedFrame>,
    current_frame: usize,
    playing: bool,
    /// Loop count: 0 = infinite, >0 = number of repetitions.
    loop_count: i32,
    /// Internal timer accumulator in milliseconds.
    frame_timer: u64,
    /// Frame delay in milliseconds (used when frame does not specify its own).
    frame_delay: u64,
    /// Number of completed loops so far.
    loops_completed: i32,
    /// Emitted when the animation finishes (all loops completed).
    pub animation_finished: GenericSignal,
    /// Emitted when the current frame index changes. Passes the new frame index.
    pub frame_changed: Signal1<u32>,
}

impl AnimatedImage {
    /// Creates a new AnimatedImage with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::AnimatedImage, geometry, "AnimatedImage"),
            frames: Vec::new(),
            current_frame: 0,
            playing: false,
            loop_count: 0,
            frame_timer: 0,
            frame_delay: 100,
            loops_completed: 0,
            animation_finished: GenericSignal::new(),
            frame_changed: Signal1::new(),
        }
    }

    /// Loads frames from raw byte data. The data is treated as a simple
    /// frame sequence for simulation purposes.
    ///
    /// In a real implementation, this would decode GIF/APNG/WebP streams.
    /// Here we create a single placeholder frame with the given dimensions.
    pub fn load_from_bytes(
        &mut self,
        data: &[u8],
        format: AnimatedImageFormat,
    ) -> Result<(), String> {
        if data.is_empty() {
            return Err("No data provided".to_string());
        }

        // For simulation, create a simple colored test pattern frame based on data hash.
        // In production, this would parse the actual image format.
        let width = 64u32;
        let height = 64u32;
        let pixel_count = (width * height) as usize;
        let mut frame_data = Vec::with_capacity(pixel_count * 4);

        // Generate a simple pattern based on the input data.
        let seed: u8 = data.iter().fold(0u8, |a, b| a.wrapping_add(*b));
        for y in 0..height {
            for x in 0..width {
                let r = (x as u8).wrapping_mul(seed);
                let g = (y as u8).wrapping_mul(seed.wrapping_add(1));
                let b = seed.wrapping_sub(x as u8);
                let a = 255u8;
                frame_data.push(r);
                frame_data.push(g);
                frame_data.push(b);
                frame_data.push(a);
            }
        }

        let delay = match format {
            AnimatedImageFormat::Gif => 100,
            AnimatedImageFormat::Apng => 40,
            AnimatedImageFormat::WebP => 33,
        };

        self.frames.push(AnimatedFrame { data: frame_data, width, height, delay_ms: delay });

        // If data is long enough, simulate a second frame.
        if data.len() > 32 {
            let mut frame_data2 = Vec::with_capacity(pixel_count * 4);
            for y in 0..height {
                for x in 0..width {
                    let r = seed.wrapping_mul(y as u8).wrapping_add(100);
                    let g = (x as u8).wrapping_mul(seed.wrapping_add(2));
                    let b = seed.wrapping_add(y as u8);
                    let a = 255u8;
                    frame_data2.push(r);
                    frame_data2.push(g);
                    frame_data2.push(b);
                    frame_data2.push(a);
                }
            }
            self.frames.push(AnimatedFrame {
                data: frame_data2,
                width,
                height,
                delay_ms: delay + 20,
            });
        }

        self.current_frame = 0;
        self.frame_timer = 0;
        self.loops_completed = 0;
        self.playing = false;
        Ok(())
    }

    /// Starts playback of the animation.
    pub fn play(&mut self) {
        if self.frames.is_empty() {
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
        self.loops_completed = 0;
        self.base.request_redraw();
    }

    /// Returns whether the animation is currently playing.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Sets the loop count. 0 = infinite, >0 = number of repetitions.
    pub fn set_loop_count(&mut self, count: i32) {
        self.loop_count = count.max(0);
    }

    /// Returns the current loop count setting.
    pub fn loop_count(&self) -> i32 {
        self.loop_count
    }

    /// Returns the current frame index.
    pub fn current_frame(&self) -> usize {
        self.current_frame
    }

    /// Returns the total number of frames.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Advances to the next frame. Wraps around when reaching the end
    /// and handles loop counting.
    pub fn advance_frame(&mut self) {
        if self.frames.is_empty() {
            return;
        }

        let next = self.current_frame + 1;
        if next >= self.frames.len() {
            // Reached the end of the sequence.
            if self.loop_count == 0 {
                // Infinite looping: wrap around.
                self.current_frame = 0;
            } else {
                self.loops_completed += 1;
                if self.loops_completed >= self.loop_count {
                    // All loops completed, stop.
                    self.playing = false;
                    self.animation_finished.emit();
                    // Stay on the last frame.
                    return;
                }
                self.current_frame = 0;
            }
        } else {
            self.current_frame = next;
        }

        self.frame_timer = 0;
        self.frame_changed.emit(self.current_frame as u32);
        self.base.request_redraw();
    }

    /// Advances the animation timer by the given number of milliseconds.
    /// Returns true if the frame changed as a result.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        if !self.playing || self.frames.is_empty() {
            return false;
        }

        let delay =
            self.frames.get(self.current_frame).map(|f| f.delay_ms).unwrap_or(self.frame_delay);

        self.frame_timer += delta_ms;
        if self.frame_timer >= delay {
            self.advance_frame();
            true
        } else {
            false
        }
    }

    /// Resets the animation to the first frame without stopping.
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.frame_timer = 0;
        self.loops_completed = 0;
        self.frame_changed.emit(0);
        self.base.request_redraw();
    }
}

impl Widget for AnimatedImage {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for AnimatedImage {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        if self.frames.is_empty() {
            // Empty state: draw a neutral placeholder.
            context.fill_rounded_rect(rect, 4, Color::rgba(230, 230, 230, 200));
            let font = crate::core::Font::default();
            let text = "No frames loaded";
            let metrics = context.measure_text(text, &font);
            let text_x = rect.x + (rect.width as i32 - metrics.width as i32) / 2;
            let text_y = rect.y + rect.height as i32 / 2 + metrics.ascent as i32 / 2;
            context.draw_text(
                Point::new(text_x, text_y),
                text,
                &font,
                Color::rgba(160, 160, 160, 220),
                HorizontalAlignment::Left,
            );
            return;
        }

        if let Some(frame) = self.frames.get(self.current_frame) {
            // Background.
            let bg = if !is_enabled { Color::rgba(200, 200, 200, 100) } else { Color::WHITE };
            context.fill_rect(rect, bg);

            // Draw the frame centered in the widget geometry.
            let fw = frame.width as i32;
            let fh = frame.height as i32;
            let dx = rect.x + (rect.width as i32 - fw) / 2;
            let dy = rect.y + (rect.height as i32 - fh) / 2;
            context.draw_image(dx.max(0), dy.max(0), frame.width, frame.height, &frame.data);

            // Draw play/pause indicator overlay if animation is stopped or paused.
            if !self.playing {
                let overlay_color = Color::rgba(0, 0, 0, 80);
                let indicator_size = 32u32;
                let ix = rect.x + (rect.width as i32 - indicator_size as i32) / 2;
                let iy = rect.y + (rect.height as i32 - indicator_size as i32) / 2;
                let indicator_rect = Rect::new(ix, iy, indicator_size, indicator_size);
                context.fill_rounded_rect(indicator_rect, 6, overlay_color);

                if self.current_frame == 0 && !self.playing {
                    // Play triangle.
                    let cx = ix + indicator_size as i32 / 2;
                    let cy = iy + indicator_size as i32 / 2;
                    let tri_size = 10i32;
                    let points = [Point::new(cx - tri_size / 2, cy - tri_size),
                        Point::new(cx - tri_size / 2, cy + tri_size),
                        Point::new(cx + tri_size / 2, cy)];
                    if let Some(first) = points.first() {
                        let mut prev = *first;
                        for p in points.iter().skip(1) {
                            context.draw_line(prev, *p, Color::WHITE);
                            prev = *p;
                        }
                        context.draw_line(prev, *first, Color::WHITE);
                    }
                }
            }

            // Frame counter overlay at top-right.
            let counter_text = format!("{}/{}", self.current_frame + 1, self.frames.len());
            let font = crate::core::Font::default();
            let metrics = context.measure_text(&counter_text, &font);
            let cx = rect.x + rect.width as i32 - metrics.width as i32 - 4;
            let cy = rect.y + 2;
            // Background pill for counter.
            let pill_w = metrics.width as u32 + 8;
            let pill_h = metrics.height as u32 + 2;
            let pill_rect = Rect::new(cx - 4, cy - 1, pill_w, pill_h);
            context.fill_rounded_rect(pill_rect, 3, Color::rgba(0, 0, 0, 60));
            context.draw_text(
                Point::new(cx, cy + metrics.ascent as i32),
                &counter_text,
                &font,
                Color::WHITE,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for AnimatedImage {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button == 1
                    && self.geometry().contains_point(*pos) {
                        if self.playing {
                            self.pause();
                        } else {
                            self.play();
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

    fn make_test_data(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 255) as u8).collect()
    }

    #[test]
    fn animated_image_creation_defaults() {
        let img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        assert_eq!(img.frame_count(), 0);
        assert_eq!(img.current_frame(), 0);
        assert!(!img.is_playing());
        assert_eq!(img.loop_count(), 0);
        assert_eq!(img.kind(), WidgetKind::AnimatedImage);
    }

    #[test]
    fn animated_image_load_from_bytes_and_frame_count() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let data = make_test_data(128);
        img.load_from_bytes(&data, AnimatedImageFormat::Gif).unwrap();
        assert_eq!(img.frame_count(), 2);
        assert_eq!(img.current_frame(), 0);
    }

    #[test]
    fn animated_image_empty_data_returns_error() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let result = img.load_from_bytes(&[], AnimatedImageFormat::Gif);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No data provided");
    }

    #[test]
    fn animated_image_play_pause_stop() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let data = make_test_data(128);
        img.load_from_bytes(&data, AnimatedImageFormat::Gif).unwrap();

        assert!(!img.is_playing());
        img.play();
        assert!(img.is_playing());
        img.pause();
        assert!(!img.is_playing());
        img.play();
        assert!(img.is_playing());
        img.stop();
        assert!(!img.is_playing());
        assert_eq!(img.current_frame(), 0);
    }

    #[test]
    fn animated_image_advance_frame_wraps_with_infinite_loop() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let data = make_test_data(128);
        img.load_from_bytes(&data, AnimatedImageFormat::Gif).unwrap();
        img.play();
        assert_eq!(img.frame_count(), 2);

        img.advance_frame();
        assert_eq!(img.current_frame(), 1);

        img.advance_frame();
        // With infinite loop (count=0), wraps back to 0.
        assert_eq!(img.current_frame(), 0);

        img.advance_frame();
        assert_eq!(img.current_frame(), 1);
    }

    #[test]
    fn animated_image_advance_frame_finite_loop() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let data = make_test_data(128);
        img.load_from_bytes(&data, AnimatedImageFormat::Gif).unwrap();
        img.set_loop_count(2);
        img.play();
        assert_eq!(img.loops_completed, 0);

        // Frame 0 -> 1
        img.advance_frame();
        assert_eq!(img.current_frame(), 1);

        // Frame 1 -> wraps, loops_completed=1, frame=0
        img.advance_frame();
        assert_eq!(img.current_frame(), 0);
        assert_eq!(img.loops_completed, 1);

        // Frame 0 -> 1
        img.advance_frame();
        assert_eq!(img.current_frame(), 1);

        // Frame 1 -> wraps, loops_completed=2, stops (should not advance past last frame)
        img.advance_frame();
        assert_eq!(img.current_frame(), 1); // stays on last frame
        assert!(!img.is_playing());
    }

    #[test]
    fn animated_image_frame_changed_signal() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let data = make_test_data(128);
        img.load_from_bytes(&data, AnimatedImageFormat::Gif).unwrap();
        img.play();

        let captured = Arc::new(Mutex::new(None));
        img.frame_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<u32>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        img.advance_frame();
        assert_eq!(*captured.lock().unwrap(), Some(1));
    }

    #[test]
    fn animated_image_animation_finished_signal() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let data = make_test_data(128);
        img.load_from_bytes(&data, AnimatedImageFormat::Gif).unwrap();
        img.set_loop_count(1);
        img.play();

        let finished = Arc::new(Mutex::new(false));
        img.animation_finished.connect({
            let finished = Arc::clone(&finished);
            move || {
                *finished.lock().unwrap() = true;
            }
        });

        // Frame 0 -> 1 (first loop).
        img.advance_frame();
        // Frame 1 -> wraps: loops_completed reaches 1 -> finish.
        img.advance_frame();
        assert!(*finished.lock().unwrap());
    }

    #[test]
    fn animated_image_tick_advances_frame() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let data = make_test_data(128);
        img.load_from_bytes(&data, AnimatedImageFormat::Gif).unwrap();
        img.play();

        let changed = img.tick(30); // below frame delay (100ms)
        assert!(!changed);
        assert_eq!(img.current_frame(), 0);

        let changed = img.tick(80); // total 110ms >= 100ms
        assert!(changed);
        assert_eq!(img.current_frame(), 1);
    }

    #[test]
    fn animated_image_tick_not_playing_does_nothing() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let data = make_test_data(128);
        img.load_from_bytes(&data, AnimatedImageFormat::Gif).unwrap();
        // Not playing.
        let changed = img.tick(500);
        assert!(!changed);
        assert_eq!(img.current_frame(), 0);
    }

    #[test]
    fn animated_image_loop_count_get_set() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        assert_eq!(img.loop_count(), 0);
        img.set_loop_count(5);
        assert_eq!(img.loop_count(), 5);
        img.set_loop_count(-1); // should clamp to 0
        assert_eq!(img.loop_count(), 0);
    }

    #[test]
    fn animated_image_disabled_blocks_events() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let data = make_test_data(128);
        img.load_from_bytes(&data, AnimatedImageFormat::Gif).unwrap();
        img.set_enabled(false);

        img.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1 });
        assert!(!img.is_playing());
    }

    #[test]
    fn animated_image_reset() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let data = make_test_data(128);
        img.load_from_bytes(&data, AnimatedImageFormat::Gif).unwrap();
        img.play();
        img.advance_frame();
        assert_eq!(img.current_frame(), 1);

        img.reset();
        assert_eq!(img.current_frame(), 0);
        assert!(img.is_playing()); // still playing
    }

    #[test]
    fn animated_image_svg_output() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 100, 100));
        let data = make_test_data(64);
        img.load_from_bytes(&data, AnimatedImageFormat::Apng).unwrap();
        let svg = crate::widget::svg::render_to_svg(&mut img);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"100\""));
        assert!(svg.ends_with("</svg>"));
    }
}
