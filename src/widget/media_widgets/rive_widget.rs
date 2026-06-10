//! RiveWidget — Rive animation runtime widget.
//!
//! The RiveWidget manages a Rive animation with state machine inputs,
//! play/pause/stop controls, and loop count configuration. It emits a
//! signal when the animation finishes.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// The value type for a Rive state machine input.
#[derive(Debug, Clone, PartialEq)]
pub enum RiveInputValue {
    /// Boolean input.
    Bool(bool),
    /// Numeric (float) input.
    Number(f32),
    /// Trigger input (one-shot).
    Trigger,
}

/// A named input for a Rive state machine.
#[derive(Debug, Clone)]
pub struct RiveInput {
    /// The name of the input.
    pub name: String,
    /// The current value of the input.
    pub value: RiveInputValue,
}

/// RiveWidget — a Rive animation runtime widget.
pub struct RiveWidget {
    base: BaseWidget,
    /// Name of the currently loaded animation.
    animation_name: String,
    /// Whether the animation is currently playing.
    is_playing: bool,
    /// State machine inputs.
    state_machine_inputs: Vec<RiveInput>,
    /// Loop count: 0 = infinite, >0 = number of repetitions.
    loop_count: i32,
    /// Internal timer accumulator in milliseconds.
    frame_timer: u64,
    /// Internal animation progress (0.0 – 1.0).
    animation_progress: f32,
    /// Emitted when the animation finishes (all loops completed).
    pub animation_finished: GenericSignal,
}

impl RiveWidget {
    /// Creates a new RiveWidget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RiveWidget, geometry, "RiveWidget"),
            animation_name: String::new(),
            is_playing: false,
            state_machine_inputs: Vec::new(),
            loop_count: 0,
            frame_timer: 0,
            animation_progress: 0.0,
            animation_finished: GenericSignal::new(),
        }
    }

    /// Loads a named animation. Replaces any previously loaded animation.
    pub fn load_animation(&mut self, name: &str) {
        self.animation_name = name.to_string();
        self.animation_progress = 0.0;
        self.frame_timer = 0;
        self.is_playing = false;
        self.base.request_redraw();
    }

    /// Returns the name of the currently loaded animation.
    pub fn animation_name(&self) -> &str {
        &self.animation_name
    }

    /// Starts playback of the animation.
    pub fn play(&mut self) {
        if self.animation_name.is_empty() {
            return;
        }
        self.is_playing = true;
        self.base.request_redraw();
    }

    /// Pauses playback, keeping the current animation state.
    pub fn pause(&mut self) {
        self.is_playing = false;
        self.base.request_redraw();
    }

    /// Stops playback and resets the animation to the beginning.
    pub fn stop(&mut self) {
        self.is_playing = false;
        self.animation_progress = 0.0;
        self.frame_timer = 0;
        self.base.request_redraw();
    }

    /// Returns whether the animation is currently playing.
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Sets the value of a named state machine input.
    /// If the input does not exist, it is created.
    pub fn set_input(&mut self, name: &str, value: RiveInputValue) {
        if let Some(input) = self.state_machine_inputs.iter_mut().find(|i| i.name == name) {
            input.value = value;
        } else {
            self.state_machine_inputs.push(RiveInput { name: name.to_string(), value });
        }
        self.base.request_redraw();
    }

    /// Returns a reference to a named state machine input, if it exists.
    pub fn get_input(&self, name: &str) -> Option<&RiveInput> {
        self.state_machine_inputs.iter().find(|i| i.name == name)
    }

    /// Returns a mutable reference to a named state machine input, if it exists.
    pub fn get_input_mut(&mut self, name: &str) -> Option<&mut RiveInput> {
        self.state_machine_inputs.iter_mut().find(|i| i.name == name)
    }

    /// Removes a named state machine input.
    pub fn remove_input(&mut self, name: &str) {
        self.state_machine_inputs.retain(|i| i.name != name);
        self.base.request_redraw();
    }

    /// Returns a reference to all state machine inputs.
    pub fn inputs(&self) -> &[RiveInput] {
        &self.state_machine_inputs
    }

    /// Sets the loop count. 0 = infinite, >0 = number of repetitions.
    pub fn set_loop_count(&mut self, n: i32) {
        self.loop_count = n.max(0);
    }

    /// Returns the current loop count.
    pub fn loop_count(&self) -> i32 {
        self.loop_count
    }

    /// Returns the current animation progress (0.0 – 1.0).
    pub fn animation_progress(&self) -> f32 {
        self.animation_progress
    }

    /// Advances the animation state by the given number of milliseconds.
    /// This simulates frame advancement based on a fixed 60fps rate.
    /// Returns true if the animation completed a loop.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        if !self.is_playing || self.animation_name.is_empty() {
            return false;
        }

        // Assume 60fps for the Rive animation, 16.67ms per tick.
        let frame_delay = 16u64;
        self.frame_timer += delta_ms;

        let mut completed = false;
        while self.frame_timer >= frame_delay {
            self.frame_timer -= frame_delay;
            let step = 1.0 / 60.0; // 1 frame at 60fps as progress fraction
            self.animation_progress += step;

            if self.animation_progress >= 1.0 {
                if self.loop_count == 0 {
                    // Infinite looping: wrap around.
                    self.animation_progress = 0.0;
                } else {
                    self.loop_count -= 1;
                    if self.loop_count == 0 {
                        // All loops completed, stop.
                        self.is_playing = false;
                        self.animation_progress = 1.0;
                        self.animation_finished.emit();
                        completed = true;
                        break;
                    }
                    self.animation_progress = 0.0;
                }
            }
        }

        self.base.request_redraw();
        completed
    }
}

impl Widget for RiveWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for RiveWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        if self.animation_name.is_empty() {
            // Empty state: draw a neutral placeholder.
            context.fill_rounded_rect(rect, 4, Color::rgba(230, 230, 230, 200));
            let font = Font::default();
            let text = "No Rive animation loaded";
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
            Color::rgba(245, 240, 250, 255)
        };
        context.fill_rect(rect, bg);

        // Draw bounding box.
        context.draw_rect_stroke(rect, Color::rgba(140, 80, 180, 150), 1);

        let font = Font::default();

        // Draw animation name.
        let name_text = format!("Rive: {}", self.animation_name);
        let name_metrics = context.measure_text(&name_text, &font);
        let name_x = rect.x + (rect.width as i32 - name_metrics.width as i32) / 2;
        let name_y = rect.y + rect.height as i32 / 4 + name_metrics.ascent as i32 / 2;
        context.draw_text(
            Point::new(name_x, name_y),
            &name_text,
            &font,
            Color::rgba(80, 40, 120, 230),
        );

        // Draw animation progress.
        let progress_text = format!("Progress: {:.1}%", self.animation_progress * 100.0);
        let progress_metrics = context.measure_text(&progress_text, &font);
        let progress_x = rect.x + (rect.width as i32 - progress_metrics.width as i32) / 2;
        let progress_y = rect.y + rect.height as i32 / 2 + progress_metrics.ascent as i32 / 2;
        context.draw_text(
            Point::new(progress_x, progress_y),
            &progress_text,
            &font,
            Color::rgba(80, 40, 120, 230),
        );

        // Draw play/pause status.
        let status = if self.is_playing { "▶ Playing" } else { "⏸ Paused" };
        let status_metrics = context.measure_text(status, &font);
        let status_x = rect.x + (rect.width as i32 - status_metrics.width as i32) / 2;
        let status_y = rect.y + rect.height as i32 * 3 / 4 + status_metrics.ascent as i32 / 2;
        let status_color = if self.is_playing {
            Color::rgba(40, 160, 40, 230)
        } else {
            Color::rgba(180, 100, 40, 230)
        };
        context.draw_text(Point::new(status_x, status_y), status, &font, status_color);

        // Draw state machine inputs if any.
        if !self.state_machine_inputs.is_empty() {
            let input_text = format!("Inputs: {}", self.state_machine_inputs.len());
            let input_metrics = context.measure_text(&input_text, &font);
            let input_x = rect.x + 4;
            let input_y = rect.y + rect.height as i32 - 4;
            context.draw_text(
                Point::new(
                    input_x,
                    input_y - input_metrics.height as i32 - input_metrics.ascent as i32,
                ),
                &input_text,
                &font,
                Color::rgba(120, 80, 160, 180),
            );
        }

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

        let filled_width =
            ((progress_bar_full.width as f64) * self.animation_progress as f64) as u32;
        if filled_width > 0 {
            let progress_bar_fill = Rect::new(
                progress_bar_full.x,
                progress_bar_full.y,
                filled_width,
                progress_bar_full.height,
            );
            context.fill_rounded_rect(progress_bar_fill, 3, Color::rgba(140, 60, 200, 200));
        }
    }
}

impl EventHandler for RiveWidget {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
                if *button == 1 {
                    if self.geometry().contains_point(*pos) {
                        if self.is_playing {
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
    use std::sync::{Arc, Mutex};

    #[test]
    fn rive_widget_creation_defaults() {
        let rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        assert_eq!(rive.animation_name(), "");
        assert!(!rive.is_playing());
        assert_eq!(rive.loop_count(), 0);
        assert_eq!(rive.animation_progress(), 0.0);
        assert!(rive.inputs().is_empty());
        assert_eq!(rive.kind(), WidgetKind::RiveWidget);
    }

    #[test]
    fn rive_widget_load_animation_and_play_pause_stop() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation("idle");
        assert_eq!(rive.animation_name(), "idle");

        assert!(!rive.is_playing());
        rive.play();
        assert!(rive.is_playing());
        rive.pause();
        assert!(!rive.is_playing());
        rive.play();
        assert!(rive.is_playing());
        rive.stop();
        assert!(!rive.is_playing());
        assert_eq!(rive.animation_progress(), 0.0);
    }

    #[test]
    fn rive_widget_set_input_and_get_input() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.set_input("enabled", RiveInputValue::Bool(true));
        rive.set_input("speed", RiveInputValue::Number(1.5));
        rive.set_input("trigger", RiveInputValue::Trigger);

        assert_eq!(rive.inputs().len(), 3);

        let enabled = rive.get_input("enabled").unwrap();
        assert_eq!(enabled.name, "enabled");
        assert_eq!(enabled.value, RiveInputValue::Bool(true));

        let speed = rive.get_input("speed").unwrap();
        assert_eq!(speed.name, "speed");
        assert_eq!(speed.value, RiveInputValue::Number(1.5));

        let trigger = rive.get_input("trigger").unwrap();
        assert_eq!(trigger.name, "trigger");
        assert_eq!(trigger.value, RiveInputValue::Trigger);
    }

    #[test]
    fn rive_widget_update_existing_input() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.set_input("volume", RiveInputValue::Number(0.5));
        rive.set_input("volume", RiveInputValue::Number(0.8));
        assert_eq!(rive.inputs().len(), 1);

        let volume = rive.get_input("volume").unwrap();
        assert_eq!(volume.value, RiveInputValue::Number(0.8));
    }

    #[test]
    fn rive_widget_tick_advances_progress() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation("run");
        rive.play();

        assert_eq!(rive.animation_progress(), 0.0);

        // Tick 16ms (one frame at 60fps).
        rive.tick(16);
        assert!(rive.animation_progress() > 0.0);
        assert!(rive.animation_progress() < 0.1);
    }

    #[test]
    fn rive_widget_tick_not_playing_does_nothing() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation("run");
        // Not playing.
        rive.tick(100);
        assert_eq!(rive.animation_progress(), 0.0);
    }

    #[test]
    fn rive_widget_animation_finished_signal() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation("walk");
        rive.set_loop_count(1);
        rive.play();

        let finished = Arc::new(Mutex::new(false));
        rive.animation_finished.connect({
            let finished = Arc::clone(&finished);
            move || {
                *finished.lock().unwrap() = true;
            }
        });

        // Tick enough to complete one loop (60 frames at 60fps ≈ 1000ms).
        rive.tick(1000);
        assert!(*finished.lock().unwrap());
        assert!(!rive.is_playing());
    }

    #[test]
    fn rive_widget_infinite_loop() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.load_animation("spin");
        rive.set_loop_count(0); // infinite
        rive.play();

        // Tick enough to loop multiple times.
        rive.tick(2000);
        assert!(rive.is_playing()); // still playing (infinite)
                                    // Progress should be somewhere in [0, 1.0)
        assert!(rive.animation_progress() >= 0.0);
        assert!(rive.animation_progress() < 1.0);
    }

    #[test]
    fn rive_widget_remove_input() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        rive.set_input("foo", RiveInputValue::Bool(true));
        rive.set_input("bar", RiveInputValue::Number(42.0));
        assert_eq!(rive.inputs().len(), 2);

        rive.remove_input("foo");
        assert_eq!(rive.inputs().len(), 1);
        assert!(rive.get_input("foo").is_none());
        assert!(rive.get_input("bar").is_some());
    }
}
