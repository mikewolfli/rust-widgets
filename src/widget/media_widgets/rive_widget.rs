//! RiveWidget — Rive animation runtime widget.
//!
//! The RiveWidget manages a Rive animation with state machine inputs,
//! play/pause/stop controls, and loop count configuration. It emits a
//! signal when the animation finishes. Animated shapes are rendered
//! based on the animation progress value.

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

// ──────────────────────────────────────────────
// Rive animation shape model
// ──────────────────────────────────────────────

/// Type of a Rive animated shape.
#[derive(Debug, Clone, Copy, PartialEq)]
enum RiveShapeType {
    Rectangle,
    Circle,
    Ellipse,
    Triangle,
    Star,
}

/// An animated shape in a Rive animation.
/// Each shape has keyframed properties that interpolate based on progress.
#[derive(Debug, Clone)]
struct RiveAnimatedShape {
    /// The shape type.
    shape_type: RiveShapeType,
    /// Base x position (center).
    x: f64,
    /// Base y position (center).
    y: f64,
    /// Base width.
    width: f64,
    /// Base height.
    height: f64,
    /// Base color (rgba as u8 values).
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    /// Motion amplitude for animation — how far the shape moves from base.
    motion_x: f64,
    motion_y: f64,
    /// Scale animation amplitude (0 = no scaling).
    scale_amplitude: f64,
    /// Rotation amplitude in degrees (0 = no rotation).
    rotation_amplitude: f64,
    /// Color shift — whether color changes with progress.
    color_shift: bool,
}

impl RiveAnimatedShape {
    /// Compute the interpolated position for a given progress value (0.0 - 1.0).
    fn position_at(&self, progress: f64) -> (f64, f64) {
        let angle = progress * std::f64::consts::TAU;
        let nx = self.x + self.motion_x * angle.sin();
        let ny = self.y + self.motion_y * angle.cos();
        (nx, ny)
    }

    /// Compute the interpolated size (width, height) for a given progress.
    fn size_at(&self, progress: f64) -> (f64, f64) {
        if self.scale_amplitude > 0.0 {
            let scale = 1.0 + self.scale_amplitude * (progress * std::f64::consts::TAU).sin();
            (self.width * scale, self.height * scale)
        } else {
            (self.width, self.height)
        }
    }

    /// Compute the color for a given progress.
    fn color_at(&self, progress: f64) -> Color {
        if self.color_shift {
            let hue_shift = (progress * 360.0) as f32;
            let r = (self.r as f32 + hue_shift * 0.3).round().clamp(0.0, 255.0) as u8;
            let g = (self.g as f32 + hue_shift * 0.2).round().clamp(0.0, 255.0) as u8;
            let b = (self.b as f32 + hue_shift * 0.1).round().clamp(0.0, 255.0) as u8;
            Color::rgba(r, g, b, self.a)
        } else {
            Color::rgba(self.r, self.g, self.b, self.a)
        }
    }

    /// Compute rotation in degrees for a given progress.
    fn rotation_at(&self, progress: f64) -> f64 {
        if self.rotation_amplitude > 0.0 {
            self.rotation_amplitude * (progress * std::f64::consts::TAU).sin()
        } else {
            0.0
        }
    }
}

/// The animation data parsed from a Rive-like JSON structure.
#[derive(Debug, Clone)]
struct RiveAnimationData {
    /// Name of the animation.
    #[allow(dead_code)]
    name: String,
    /// Duration of the animation in frames.
    #[allow(dead_code)]
    duration_frames: u32,
    /// Frame rate.
    #[allow(dead_code)]
    frame_rate: f32,
    /// Shapes in this animation.
    shapes: Vec<RiveAnimatedShape>,
}

impl RiveAnimationData {
    /// Parse from a JSON value that describes the animation.
    fn from_json(name: &str, val: &serde_json::Value) -> Self {
        let duration_frames = val.get("duration").and_then(|v| v.as_u64()).unwrap_or(60) as u32;
        let frame_rate = val.get("fr").and_then(|v| v.as_f64()).unwrap_or(60.0) as f32;

        let shapes = if let Some(shapes_arr) = val.get("shapes").and_then(|v| v.as_array()) {
            shapes_arr
                .iter()
                .map(|s| {
                    let shape_type_str = s.get("ty").and_then(|v| v.as_str()).unwrap_or("rect");
                    let shape_type = match shape_type_str {
                        "circle" => RiveShapeType::Circle,
                        "ellipse" => RiveShapeType::Ellipse,
                        "triangle" => RiveShapeType::Triangle,
                        "star" => RiveShapeType::Star,
                        _ => RiveShapeType::Rectangle,
                    };
                    let x = s.get("x").and_then(|v| v.as_f64()).unwrap_or(50.0);
                    let y = s.get("y").and_then(|v| v.as_f64()).unwrap_or(50.0);
                    let width = s.get("w").and_then(|v| v.as_f64()).unwrap_or(40.0);
                    let height = s.get("h").and_then(|v| v.as_f64()).unwrap_or(40.0);
                    let r = s.get("r").and_then(|v| v.as_u64()).unwrap_or(80) as u8;
                    let g = s.get("g").and_then(|v| v.as_u64()).unwrap_or(60) as u8;
                    let b = s.get("b").and_then(|v| v.as_u64()).unwrap_or(180) as u8;
                    let a = s.get("a").and_then(|v| v.as_u64()).unwrap_or(220) as u8;
                    let motion_x = s.get("mx").and_then(|v| v.as_f64()).unwrap_or(10.0);
                    let motion_y = s.get("my").and_then(|v| v.as_f64()).unwrap_or(5.0);
                    let scale_amp = s.get("sa").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let rot_amp = s.get("ra").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let color_shift = s.get("cs").and_then(|v| v.as_bool()).unwrap_or(false);
                    RiveAnimatedShape {
                        shape_type,
                        x,
                        y,
                        width,
                        height,
                        r,
                        g,
                        b,
                        a,
                        motion_x,
                        motion_y,
                        scale_amplitude: scale_amp,
                        rotation_amplitude: rot_amp,
                        color_shift,
                    }
                })
                .collect()
        } else {
            // Default shapes if none specified: a bouncing rect and a pulsing circle.
            vec![
                RiveAnimatedShape {
                    shape_type: RiveShapeType::Rectangle,
                    x: 50.0,
                    y: 50.0,
                    width: 60.0,
                    height: 60.0,
                    r: 60,
                    g: 100,
                    b: 200,
                    a: 220,
                    motion_x: 20.0,
                    motion_y: 15.0,
                    scale_amplitude: 0.2,
                    rotation_amplitude: 15.0,
                    color_shift: false,
                },
                RiveAnimatedShape {
                    shape_type: RiveShapeType::Circle,
                    x: 50.0,
                    y: 50.0,
                    width: 30.0,
                    height: 30.0,
                    r: 200,
                    g: 80,
                    b: 60,
                    a: 200,
                    motion_x: -15.0,
                    motion_y: 10.0,
                    scale_amplitude: 0.3,
                    rotation_amplitude: 0.0,
                    color_shift: true,
                },
            ]
        };

        RiveAnimationData { name: name.to_string(), duration_frames, frame_rate, shapes }
    }

    /// Create default animation data for a named animation with no JSON.
    fn default_for(name: &str) -> Self {
        RiveAnimationData {
            name: name.to_string(),
            duration_frames: 60,
            frame_rate: 60.0,
            shapes: vec![
                RiveAnimatedShape {
                    shape_type: RiveShapeType::Rectangle,
                    x: 50.0,
                    y: 50.0,
                    width: 60.0,
                    height: 60.0,
                    r: 60,
                    g: 100,
                    b: 200,
                    a: 220,
                    motion_x: 20.0,
                    motion_y: 15.0,
                    scale_amplitude: 0.2,
                    rotation_amplitude: 15.0,
                    color_shift: false,
                },
                RiveAnimatedShape {
                    shape_type: RiveShapeType::Circle,
                    x: 50.0,
                    y: 50.0,
                    width: 30.0,
                    height: 30.0,
                    r: 200,
                    g: 80,
                    b: 60,
                    a: 200,
                    motion_x: -15.0,
                    motion_y: 10.0,
                    scale_amplitude: 0.3,
                    rotation_amplitude: 0.0,
                    color_shift: true,
                },
            ],
        }
    }
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
    /// Parsed animation data with shapes to render.
    animation_data: Option<RiveAnimationData>,
    /// Raw animation JSON data (optional).
    animation_json: Option<String>,
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
            animation_data: None,
            animation_json: None,
        }
    }

    /// Loads a named animation. Replaces any previously loaded animation.
    /// If `json_data` is provided, it will be parsed for custom shapes.
    pub fn load_animation(&mut self, name: &str) {
        self.animation_name = name.to_string();
        self.animation_progress = 0.0;
        self.frame_timer = 0;
        self.is_playing = false;
        self.animation_data = Some(RiveAnimationData::default_for(name));
        self.base.request_redraw();
    }

    /// Loads a named animation from JSON data.
    /// The JSON should contain shape definitions and animation parameters.
    pub fn load_animation_from_json(&mut self, name: &str, json_data: &str) -> Result<(), String> {
        if json_data.is_empty() {
            return Err("Animation JSON data is empty".to_string());
        }
        let parsed: serde_json::Value = serde_json::from_str(json_data)
            .map_err(|e| format!("Invalid Rive animation JSON: {}", e))?;

        self.animation_name = name.to_string();
        self.animation_progress = 0.0;
        self.frame_timer = 0;
        self.is_playing = false;
        self.animation_data = Some(RiveAnimationData::from_json(name, &parsed));
        self.animation_json = Some(json_data.to_string());
        self.base.request_redraw();
        Ok(())
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

fn draw_shape(
    context: &mut RenderContext,
    shape_type: RiveShapeType,
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    color: Color,
    rotation_deg: f64,
) {
    if w <= 1.0 || h <= 1.0 {
        return;
    }
    let iw = w.round() as u32;
    let ih = h.round() as u32;
    let has_rotation = rotation_deg.abs() > 0.5;

    match shape_type {
        RiveShapeType::Rectangle => {
            let ex = (cx - w / 2.0).round() as i32;
            let ey = (cy - h / 2.0).round() as i32;
            let shape_rect = Rect::new(ex, ey, iw, ih);
            if has_rotation {
                // Draw with a subtle visual hint of rotation using corner radius.
                let corner = (iw.min(ih) / 6).max(1);
                context.fill_rounded_rect(shape_rect, corner, color);
            } else {
                context.fill_rect(shape_rect, color);
            }
        }
        RiveShapeType::Circle => {
            let radius = (iw.min(ih) / 2).max(1);
            context.fill_circle(Point::new(cx.round() as i32, cy.round() as i32), radius, color);
        }
        RiveShapeType::Ellipse => {
            let radius = (iw.min(ih) / 2).max(1);
            let ex = (cx - w / 2.0).round() as i32;
            let ey = (cy - h / 2.0).round() as i32;
            let shape_rect = Rect::new(ex, ey, iw, ih);
            context.fill_rounded_rect(shape_rect, radius, color);
        }
        RiveShapeType::Triangle => {
            // Draw triangle as a filled shape approximated via the rasterizer.
            // We use a centered rounded rect as a stand-in for triangle rendering
            // since the renderer doesn't have native triangle support.
            let ex = (cx - w / 2.0).round() as i32;
            let ey = (cy - h / 2.0).round() as i32;
            let shape_rect = Rect::new(ex, ey, iw, ih);
            context.fill_rect(shape_rect, color);
            // Draw diagonal hints to suggest a triangle shape.
            let p1 = Point::new(ex, ey + ih as i32);
            let p2 = Point::new(ex + iw as i32, ey);
            context.draw_line_stroke(p1, p2, color, 2);
        }
        RiveShapeType::Star => {
            // Draw a star approximated by overlapping circles and a central rect.
            let radius = (iw.min(ih) / 3).max(1);
            let cx_i32 = cx.round() as i32;
            let cy_i32 = cy.round() as i32;
            context.fill_circle(Point::new(cx_i32, cy_i32), radius, color);
            context.fill_circle(Point::new(cx_i32 - radius as i32, cy_i32), radius / 2, color);
            context.fill_circle(Point::new(cx_i32 + radius as i32, cy_i32), radius / 2, color);
            context.fill_circle(Point::new(cx_i32, cy_i32 - radius as i32), radius / 2, color);
            context.fill_circle(Point::new(cx_i32, cy_i32 + radius as i32), radius / 2, color);
        }
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

        // Calculate scale from composition (default 100x100) to widget rect.
        let (comp_w, comp_h) = match self.animation_data {
            Some(ref data) => {
                if data.shapes.is_empty() {
                    (100.0, 100.0)
                } else {
                    // Compute bounds from shapes.
                    let max_extent = data
                        .shapes
                        .iter()
                        .map(|s| {
                            let mx = (s.x + s.motion_x).abs().max((s.x - s.motion_x).abs())
                                + s.width.abs()
                                + 20.0;
                            let my = (s.y + s.motion_y).abs().max((s.y - s.motion_y).abs())
                                + s.height.abs()
                                + 20.0;
                            mx.max(my)
                        })
                        .fold(100.0_f64, |a, b| a.max(b));
                    (max_extent * 2.0, max_extent * 2.0)
                }
            }
            None => (100.0, 100.0),
        };

        let scale_x = rect.width as f64 / comp_w.max(1.0);
        let scale_y = rect.height as f64 / comp_h.max(1.0);
        let scale = scale_x.min(scale_y);
        let offset_x = rect.x as f64 + (rect.width as f64 - comp_w * scale) / 2.0;
        let offset_y = rect.y as f64 + (rect.height as f64 - comp_h * scale) / 2.0;

        let progress = self.animation_progress as f64;

        // Render animated shapes.
        if let Some(ref data) = self.animation_data {
            for shape in &data.shapes {
                let (sx, sy) = shape.position_at(progress);
                let (sw, sh) = shape.size_at(progress);
                let color = shape.color_at(progress);
                let rot = shape.rotation_at(progress);

                let cx = offset_x + sx * scale;
                let cy = offset_y + sy * scale;
                let w = sw * scale;
                let h = sh * scale;

                draw_shape(context, shape.shape_type, cx, cy, w, h, color, rot);
            }
        }

        // Draw animation name label at top.
        let font = Font::default();
        let name_text = format!("Rive: {}", self.animation_name);
        let name_metrics = context.measure_text(&name_text, &font);
        let name_x = rect.x + 4;
        let name_y = rect.y + 2 + name_metrics.ascent as i32;
        let name_bg = Rect::new(
            name_x - 2,
            rect.y + 1,
            name_metrics.width as u32 + 8,
            name_metrics.height as u32 + 4,
        );
        context.fill_rounded_rect(name_bg, 3, Color::rgba(0, 0, 0, 50));
        context.draw_text(Point::new(name_x, name_y), &name_text, &font, Color::WHITE);

        // Draw progress percentage at top-right.
        let progress_text = format!("{:.0}%", self.animation_progress * 100.0);
        let p_metrics = context.measure_text(&progress_text, &font);
        let px = rect.x + rect.width as i32 - p_metrics.width as i32 - 6;
        let py = rect.y + 2 + p_metrics.ascent as i32;
        let p_bg =
            Rect::new(px - 2, rect.y + 1, p_metrics.width as u32 + 8, p_metrics.height as u32 + 4);
        context.fill_rounded_rect(p_bg, 3, Color::rgba(0, 0, 0, 50));
        context.draw_text(Point::new(px, py), &progress_text, &font, Color::WHITE);

        // Play/pause icon.
        let status = if self.is_playing { "▶" } else { "⏸" };
        context.draw_text(
            Point::new(rect.x + 4, rect.y + rect.height as i32 - 4),
            status,
            &font,
            if self.is_playing {
                Color::rgba(40, 160, 40, 200)
            } else {
                Color::rgba(180, 100, 40, 200)
            },
        );

        // State machine inputs count.
        if !self.state_machine_inputs.is_empty() {
            let input_text = format!("Inputs: {}", self.state_machine_inputs.len());
            let input_metrics = context.measure_text(&input_text, &font);
            let input_x = rect.x + rect.width as i32 - input_metrics.width as i32 - 6;
            let input_y = rect.y + rect.height as i32 - 6;
            context.draw_text(
                Point::new(input_x, input_y - input_metrics.ascent as i32),
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
            rect.width.saturating_sub(8),
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
                if *button == 1 && self.geometry().contains_point(*pos) {
                    if self.is_playing {
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
    use std::sync::{Arc, Mutex};

    fn make_rive_json() -> String {
        r#"{
            "duration": 60,
            "fr": 60,
            "shapes": [
                {"ty":"rect","x":50,"y":50,"w":60,"h":60,"r":60,"g":100,"b":200,"a":220,"mx":20,"my":15,"sa":0.2,"ra":15},
                {"ty":"circle","x":50,"y":50,"w":30,"h":30,"r":200,"g":80,"b":60,"a":200,"mx":-15,"my":10,"sa":0.3,"cs":true}
            ]
        }"#
        .to_string()
    }

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

    #[test]
    fn rive_widget_load_from_json() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        let json = make_rive_json();
        rive.load_animation_from_json("test", &json).unwrap();
        assert_eq!(rive.animation_name(), "test");
        // Should have parsed the 2 shapes from JSON.
        assert!(rive.animation_data.is_some());
    }

    #[test]
    fn rive_widget_load_from_empty_json_returns_error() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        let result = rive.load_animation_from_json("test", "");
        assert!(result.is_err());
    }

    #[test]
    fn rive_widget_load_from_invalid_json_returns_error() {
        let mut rive = RiveWidget::new(Rect::new(0, 0, 200, 200));
        let result = rive.load_animation_from_json("test", "not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid Rive animation JSON"));
    }

    #[test]
    fn rive_widget_position_at_varies_with_progress() {
        let shape = RiveAnimatedShape {
            shape_type: RiveShapeType::Rectangle,
            x: 50.0,
            y: 50.0,
            width: 40.0,
            height: 40.0,
            r: 100,
            g: 100,
            b: 100,
            a: 255,
            motion_x: 20.0,
            motion_y: 10.0,
            scale_amplitude: 0.0,
            rotation_amplitude: 0.0,
            color_shift: false,
        };
        // At progress 0, sin(0) = 0, cos(0) = 1
        let (x0, y0) = shape.position_at(0.0);
        assert!((x0 - 50.0).abs() < 0.01);
        assert!((y0 - 60.0).abs() < 0.01);

        // At progress 0.25, sin(pi/2) = 1, cos(pi/2) = 0
        let (x25, y25) = shape.position_at(0.25);
        assert!((x25 - 70.0).abs() < 0.01);
        assert!((y25 - 50.0).abs() < 0.01);
    }

    #[test]
    fn rive_widget_draw_does_not_panic_with_shapes() {
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(100, 100), 1.0);
        let mut ctx = crate::render::RenderContext::new(&mut backend);

        let mut rive = RiveWidget::new(Rect::new(0, 0, 100, 100));
        rive.load_animation("test");
        rive.play();

        // Should not panic when drawing with shapes.
        rive.draw(&mut ctx);
        // No crash = test passes.
    }

    #[test]
    fn rive_widget_draw_empty_no_crash() {
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(100, 100), 1.0);
        let mut ctx = crate::render::RenderContext::new(&mut backend);

        let mut rive = RiveWidget::new(Rect::new(0, 0, 100, 100));
        // No animation loaded - empty state.
        rive.draw(&mut ctx);
        // No crash = test passes.
    }

    #[test]
    fn rive_widget_draw_with_json_shapes_no_crash() {
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(100, 100), 1.0);
        let mut ctx = crate::render::RenderContext::new(&mut backend);

        let mut rive = RiveWidget::new(Rect::new(0, 0, 100, 100));
        let json = make_rive_json();
        rive.load_animation_from_json("test", &json).unwrap();
        rive.play();
        rive.draw(&mut ctx);
        // No crash = test passes.
    }

    #[test]
    fn rive_animated_shape_size_scales_with_progress() {
        let shape = RiveAnimatedShape {
            shape_type: RiveShapeType::Circle,
            x: 50.0,
            y: 50.0,
            width: 40.0,
            height: 40.0,
            r: 255,
            g: 0,
            b: 0,
            a: 255,
            motion_x: 0.0,
            motion_y: 0.0,
            scale_amplitude: 0.5,
            rotation_amplitude: 0.0,
            color_shift: false,
        };
        // At t=0, sin(0)=0 -> scale = 1.0, so w=40
        let (w0, _h0) = shape.size_at(0.0);
        assert!((w0 - 40.0).abs() < 0.01);
        // At t=0.25, sin(pi/2)=1 -> scale = 1.5, so w=60
        let (w25, _h25) = shape.size_at(0.25);
        assert!((w25 - 60.0).abs() < 0.01);
    }
}
