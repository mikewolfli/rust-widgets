// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AnimatedImage widget — plays caller-provided animation frame sequences.
//!
//! The widget manages a sequence of [`AnimatedFrame`]s (raw RGBA + delay) with
//! play/pause/stop controls, loop-count configuration, and signals when the
//! animation finishes or the current frame changes.
//!
//! **Decoding note**: GIF/APNG/WebP *stream* decoding is not built in. Decode
//! the container yourself (per frame) and hand the RGBA frames to
//! [`AnimatedImage::load_frames`]. [`AnimatedImage::load_from_bytes`] refuses
//! raw streams rather than fabricating placeholder frames.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
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

    /// Replace the frame sequence with caller-decoded RGBA frames.
    ///
    /// Each frame carries its own `delay_ms`. Returns `Err` for an empty list.
    pub fn load_frames(&mut self, frames: Vec<AnimatedFrame>) -> Result<(), String> {
        if frames.is_empty() {
            return Err(format!(
                "an animated image needs at least one frame, got {}; pass the decoded frames \
                 from `decode_animation` instead of an empty list",
                frames.len()
            ));
        }
        self.frames = frames;
        self.current_frame = 0;
        self.frame_timer = 0;
        self.loops_completed = 0;
        self.playing = false;
        self.base.request_redraw();
        Ok(())
    }

    /// Load frames from raw GIF/APNG/WebP bytes.
    ///
    /// This widget does not include container decoders, so raw streams are
    /// rejected with an explicit error instead of fabricating placeholder
    /// frames. Decode the stream per frame and use [`Self::load_frames`].
    pub fn load_from_bytes(
        &mut self,
        _data: &[u8],
        format: AnimatedImageFormat,
    ) -> Result<(), String> {
        Err(format!(
            "decoding {format:?} streams is not implemented; decode frames and pass them via AnimatedImage::load_frames"
        ))
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

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(100, 100)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `AnimatedImage` implements `Draw`, so `Some(self)` is total and cannot be wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    // The trait contract takes `u32` milliseconds; this control's own `tick` takes `u64` so a
    // long animation cannot wrap its clock. Widening at the boundary keeps the trait uniform.
    fn tick(&mut self, delta_ms: u32) -> bool {
        AnimatedImage::tick(self, u64::from(delta_ms))
    }

    fn is_animating(&self) -> bool {
        self.is_playing()
    }
}

/// `AnimatedImage`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_media.in.rs` / `access_write_media.in.rs` dispatch: `playing` is
/// a boolean that maps onto `play` / `pause`.
impl WidgetProperties for AnimatedImage {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "playing" => Ok(CapabilityValue::Bool(self.is_playing())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "playing" => {
                if expect_bool(value)? {
                    self.play();
                } else {
                    self.pause();
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["playing", BASE_PROPERTY_NAMES]
    }

    /// Runs the published playback commands.
    ///
    /// `set_playing` names no property: playback state is exposed as the read-only
    /// `playing`, and starting it is the control's own [`Self::play`]. The default
    /// `set_foo` convention would look for a `playing` property and, finding it but
    /// with no payload route, have told the caller to supply a value the property
    /// route then refuses. The command is real here, so an override is the honest
    /// shape.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_playing" => {
                self.play();
                Ok(())
            }
            _ => self.default_command(name),
        }
    }
}

impl Draw for AnimatedImage {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then fall back to the original literal. The literal step is
        // kept deliberately: an inactive theme must still give the widget a defined
        // appearance, and the value is the one this widget painted before, so an existing
        // pixel baseline cannot move. Resolved once per draw, because the empty state, the
        // background, the status chips and the counter all read from it and re-resolving
        // would take the theme lock several times inside one draw.
        let style = self.style().clone();
        let themed = crate::style::resolved_theme_style("animated_image");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let themed_border = themed.as_ref().and_then(|resolved| resolved.border_color);
        let themed_text = themed.as_ref().and_then(|resolved| resolved.text_color);
        let base_bg =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(230, 230, 230, 200));
        let border_color =
            style.border_color.or(themed_border).unwrap_or(Color::rgba(160, 160, 160, 200));
        // The placeholder ink follows the theme's foreground so it stays legible on whichever
        // surface the theme painted; the literal is only the last resort.
        let placeholder_text =
            style.text_color.or(themed_text).unwrap_or(Color::rgba(160, 160, 160, 220));

        if self.frames.is_empty() {
            // Empty state: a neutral placeholder panel. The panel and the label are this
            // widget's chrome — there is no frame data here at all — so both now follow the
            // theme instead of pinning the control to one appearance.
            context.fill_rounded_rect(rect, 4, base_bg);
            let font = crate::core::Font::default();
            let text = "No frames loaded";
            // Centred on the vertical midline and **fitted** to the control's width. The
            // label is 16 characters at 14 px, so it is wider than a control narrower than
            // 224 px; without the fit it ran past the panel and, in the SVG snapshot, past
            // the picture. The glyph origin is the glyph's top (`ascent` is inside the line
            // box, not above it), so half the height is the right centring offset.
            let metrics = context.measure_text(text, &font);
            let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;
            let line = Rect::new(rect.x, text_y, rect.width, metrics.height);
            context.draw_text_fitted(
                line,
                text,
                &font,
                placeholder_text,
                HorizontalAlignment::Center,
            );
            return;
        }

        if let Some(frame) = self.frames.get(self.current_frame) {
            // Background — the surface the frame is matted onto, i.e. chrome. The former
            // literal white is the fallback, and disabled is now *derived* from the resolved
            // colour rather than being a second fixed grey, so the two states stay
            // distinguishable in any theme.
            let bg = if !is_enabled { base_bg.blend(&Color::WHITE, 0.35) } else { base_bg };
            context.fill_rect(rect, bg);

            // Draw the frame centered in the widget geometry.
            let fw = frame.width as i32;
            let fh = frame.height as i32;
            let dx = rect.x + (rect.width as i32 - fw) / 2;
            let dy = rect.y + (rect.height as i32 - fh) / 2;
            // Content, not chrome: the frame's own pixels are the caller's data — the whole
            // subject of an animated image — so they are painted exactly as decoded and must
            // never be recoloured from a theme.
            //
            // Clipped to the control: a frame larger than the surface (a 640x480 GIF in a
            // 240x120 cell) was centred and then painted at full size, so it covered whatever
            // the layout put beside the control. Nothing clips a widget at this layer, so the
            // clip is what keeps a decoded frame inside the picture that carries it.
            context.push_clip(rect.x, rect.y, rect.width, rect.height);
            context.draw_image(dx.max(0), dy.max(0), frame.width, frame.height, &frame.data);
            context.pop_clip();

            // Draw play/pause indicator overlay if animation is stopped or paused.
            if !self.playing {
                // The indicator chip is a status overlay rather than part of any frame, so the
                // theme sets its tone. Only the alpha is carried across: the badge must stay
                // dark enough to read as a scrim over arbitrary frame data.
                let overlay_color = border_color;
                let indicator_size = 32u32;
                let ix = rect.x + (rect.width as i32 - indicator_size as i32) / 2;
                let iy = rect.y + (rect.height as i32 - indicator_size as i32) / 2;
                let indicator_rect = Rect::new(ix, iy, indicator_size, indicator_size);
                context.fill_rounded_rect(indicator_rect, 6, overlay_color);

                if self.current_frame == 0 && !self.playing {
                    // Play triangle. Deliberately not themed: it is a media glyph drawn on the
                    // chip above, and a themed light-on-light pair would be invisible.
                    let cx = ix + indicator_size as i32 / 2;
                    let cy = iy + indicator_size as i32 / 2;
                    let tri_size = 10i32;
                    let points = [
                        Point::new(cx - tri_size / 2, cy - tri_size),
                        Point::new(cx - tri_size / 2, cy + tri_size),
                        Point::new(cx + tri_size / 2, cy),
                    ];
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
            // Same rule as the play chip: the counter's backdrop belongs to the widget's
            // chrome, but the white counter ink and the chip's own alpha are kept as they
            // were, because they are the badge's legibility contract over the frame.
            context.fill_rounded_rect(pill_rect, 3, border_color);
            // Glyph origin is the box's top edge, so it is already `cy`; the old
            // `+ ascent` pushed the counter half a line down, out through the pill.
            context.draw_text(
                Point::new(cx, cy),
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
                if *button == 1 && self.geometry().contains_point(*pos) {
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

    /// Two 2-frame sequences for playback/advance tests.
    fn two_test_frames() -> Vec<AnimatedFrame> {
        vec![
            AnimatedFrame { data: vec![10u8; 16], width: 2, height: 2, delay_ms: 100 },
            AnimatedFrame { data: vec![20u8; 16], width: 2, height: 2, delay_ms: 100 },
        ]
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
    fn animated_image_load_frames_sets_sequence() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let frames = vec![
            AnimatedFrame { data: vec![0u8; 16], width: 2, height: 2, delay_ms: 100 },
            AnimatedFrame { data: vec![1u8; 16], width: 2, height: 2, delay_ms: 50 },
        ];
        img.load_frames(frames).unwrap();
        assert_eq!(img.frame_count(), 2);
        assert_eq!(img.current_frame(), 0);
    }

    #[test]
    fn animated_image_load_frames_rejects_empty() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        assert!(img.load_frames(Vec::new()).is_err());
    }

    #[test]
    fn animated_image_load_from_bytes_rejects_raw_streams() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let data = make_test_data(128);
        // Raw GIF/APNG/WebP bytes are refused (no built-in container decoder),
        // never silently turned into placeholder frames.
        let result = img.load_from_bytes(&data, AnimatedImageFormat::Gif);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("not implemented"), "{msg}");
        assert!(msg.contains("load_frames"), "{msg}");
        assert_eq!(img.frame_count(), 0);
    }

    #[test]
    fn animated_image_play_pause_stop() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let frames =
            vec![AnimatedFrame { data: vec![0u8; 16], width: 2, height: 2, delay_ms: 100 }];
        img.load_frames(frames).unwrap();

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
        img.load_frames(two_test_frames()).unwrap();
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
        img.load_frames(two_test_frames()).unwrap();
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
        img.load_frames(two_test_frames()).unwrap();
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
        img.load_frames(two_test_frames()).unwrap();
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
        img.load_frames(two_test_frames()).unwrap();
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
        img.load_frames(two_test_frames()).unwrap();
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
        img.load_frames(two_test_frames()).unwrap();
        img.set_enabled(false);

        img.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1 });
        assert!(!img.is_playing());
    }

    #[test]
    fn animated_image_reset() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        img.load_frames(two_test_frames()).unwrap();
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
        img.load_frames(vec![AnimatedFrame {
            data: vec![0u8; 4],
            width: 1,
            height: 1,
            delay_ms: 100,
        }])
        .unwrap();
        let svg = crate::widget::svg::render_to_svg(&mut img);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"100\""));
        assert!(svg.ends_with("</svg>"));
    }
}
