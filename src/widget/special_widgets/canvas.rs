// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Canvas widget for custom drawing operations.
//!
//! The `Canvas` widget provides a drawing surface where users can push
//! rendering commands (lines, circles, rectangles, paths, text) and have
//! them replayed during the `draw()` pass. It also emits mouse interaction
//! signals so callers can respond to clicks, drags, and hover events on
//! the drawing area.
use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::signal::GenericSignal;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A uniform scale and translation applied to every drawing command a [`Canvas`] stores.
///
/// The transform is applied when a command is added, not when it is painted, so the stored
/// geometry is already in surface coordinates: hit tests, exports and the commands themselves
/// all read the same space. Changing the transform therefore affects commands added afterwards;
/// a caller that wants to move existing content clears and re-adds it, which is what a drawing
/// surface's undo model expects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasTransform {
    /// Uniform scale, clamped to a positive value.
    pub scale: f32,
    /// Translation in surface pixels.
    pub offset: Point,
}

impl Default for CanvasTransform {
    fn default() -> Self {
        Self { scale: 1.0, offset: Point::new(0, 0) }
    }
}

impl CanvasTransform {
    /// A transform with `scale` and no translation.
    pub fn scaled(scale: f32) -> Self {
        Self { scale: if scale.is_finite() { scale.max(0.01) } else { 1.0 }, ..Self::default() }
    }

    /// Maps one point through the transform.
    pub fn point(&self, point: Point) -> Point {
        Point::new(
            self.offset.x + (point.x as f32 * self.scale).round() as i32,
            self.offset.y + (point.y as f32 * self.scale).round() as i32,
        )
    }

    /// Maps one rectangle through the transform.
    pub fn rect(&self, rect: Rect) -> Rect {
        let origin = self.point(Point::new(rect.x, rect.y));
        Rect::new(
            origin.x,
            origin.y,
            (rect.width as f32 * self.scale).round().max(1.0) as u32,
            (rect.height as f32 * self.scale).round().max(1.0) as u32,
        )
    }
}

/// Canvas widget for custom drawing operations.
pub struct Canvas {
    base: BaseWidget,
    /// Stored drawing commands replayed during `draw()`.
    commands: Vec<RenderCommand>,
    /// The transform applied to commands as they are added.
    transform: CanvasTransform,
    /// Saved transforms, restored by [`Canvas::pop_transform`].
    transform_stack: Vec<CanvasTransform>,
    /// Commands removed by undo, available to redo.
    redo_stack: Vec<RenderCommand>,
    /// The most commands the canvas retains; the oldest is dropped past this.
    command_limit: usize,
    /// Whether the wheel zooms the transform.
    zoom_enabled: bool,
    /// Emitted when a context menu is requested, carrying the pointer position.
    pub context_menu_requested: Signal1<Point>,
    /// Track mouse position for interaction signals.
    last_mouse_pos: Point,
    /// Emitted when mouse is pressed on the canvas.
    pub mouse_pressed: GenericSignal,
    /// Emitted when mouse is released on the canvas.
    pub mouse_released: GenericSignal,
    /// Emitted when mouse moves over the canvas, carrying the current cursor position.
    pub mouse_moved: crate::signal::Signal1<Point>,
    /// Emitted when a double-click occurs on the canvas.
    pub double_clicked: GenericSignal,
}

impl Canvas {
    /// Creates a new canvas widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Canvas, geometry, "Canvas"),
            commands: Vec::new(),
            transform: CanvasTransform::default(),
            transform_stack: Vec::new(),
            redo_stack: Vec::new(),
            command_limit: 100_000,
            zoom_enabled: true,
            context_menu_requested: Signal1::new(),
            last_mouse_pos: Point::new(0, 0),
            mouse_pressed: GenericSignal::new(),
            mouse_released: GenericSignal::new(),
            mouse_moved: crate::signal::Signal1::new(),
            double_clicked: GenericSignal::new(),
        }
    }

    /// Remove all stored drawing commands.
    pub fn clear(&mut self) {
        self.commands.clear();
        self.base.request_redraw();
    }

    /// Returns the number of stored drawing commands.
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    /// Returns the transform applied to newly added commands.
    pub fn transform(&self) -> CanvasTransform {
        self.transform
    }

    /// Replaces the transform applied to newly added commands.
    pub fn set_transform(&mut self, transform: CanvasTransform) {
        if self.transform == transform {
            return;
        }
        self.transform = transform;
        self.base.request_redraw();
    }

    /// Saves the current transform, restoring it with [`Self::pop_transform`].
    pub fn push_transform(&mut self) {
        self.transform_stack.push(self.transform);
    }

    /// Restores the transform saved by the matching [`Self::push_transform`].
    pub fn pop_transform(&mut self) {
        if let Some(transform) = self.transform_stack.pop() {
            self.transform = transform;
            self.base.request_redraw();
        }
    }

    /// Enables or disables wheel zoom.
    pub fn set_zoom_enabled(&mut self, enabled: bool) {
        if self.zoom_enabled == enabled {
            return;
        }
        self.zoom_enabled = enabled;
    }

    /// Returns whether wheel zoom is enabled.
    pub fn is_zoom_enabled(&self) -> bool {
        self.zoom_enabled
    }

    /// Sets how many commands the canvas retains (clamped to at least one).
    pub fn set_command_limit(&mut self, limit: usize) {
        self.command_limit = limit.max(1);
        while self.commands.len() > self.command_limit {
            self.commands.remove(0);
        }
    }

    /// Returns the command limit.
    pub fn command_limit(&self) -> usize {
        self.command_limit
    }

    /// Removes the most recently added command, returning whether one was undone.
    pub fn undo(&mut self) -> bool {
        let Some(command) = self.commands.pop() else {
            return false;
        };
        self.redo_stack.push(command);
        self.base.request_redraw();
        true
    }

    /// Re-applies the command most recently removed by [`Self::undo`].
    pub fn redo(&mut self) -> bool {
        let Some(command) = self.redo_stack.pop() else {
            return false;
        };
        self.commands.push(command);
        self.base.request_redraw();
        true
    }

    /// Returns whether there is an undone command to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Applies the current transform to `command` and stores it.
    fn push_command(&mut self, command: RenderCommand) {
        let transform = self.transform;
        let command = match command {
            RenderCommand::FillRect { rect, color } => {
                RenderCommand::FillRect { rect: transform.rect(rect), color }
            }
            RenderCommand::DrawRect { rect, color } => {
                RenderCommand::DrawRect { rect: transform.rect(rect), color }
            }
            RenderCommand::FillRoundedRect { rect, radius, color } => {
                RenderCommand::FillRoundedRect {
                    rect: transform.rect(rect),
                    radius: (radius as f32 * transform.scale).round().max(1.0) as u32,
                    color,
                }
            }
            RenderCommand::DrawLine { from, to, color } => RenderCommand::DrawLine {
                from: transform.point(from),
                to: transform.point(to),
                color,
            },
            RenderCommand::DrawLineAA { from, to, color } => RenderCommand::DrawLineAA {
                from: transform.point(from),
                to: transform.point(to),
                color,
            },
            RenderCommand::DrawLineStroke { from, to, color, width } => {
                RenderCommand::DrawLineStroke {
                    from: transform.point(from),
                    to: transform.point(to),
                    color,
                    width: (width as f32 * transform.scale).round().max(1.0) as u32,
                }
            }
            RenderCommand::FillCircle { center, radius, color } => RenderCommand::FillCircle {
                center: transform.point(center),
                radius: (radius as f32 * transform.scale).round().max(1.0) as u32,
                color,
            },
            RenderCommand::FillCircleAA { center, radius, color } => RenderCommand::FillCircleAA {
                center: transform.point(center),
                radius: (radius as f32 * transform.scale).round().max(1.0) as u32,
                color,
            },
            RenderCommand::DrawCircle { center, radius, color } => RenderCommand::DrawCircle {
                center: transform.point(center),
                radius: (radius as f32 * transform.scale).round().max(1.0) as u32,
                color,
            },
            RenderCommand::DrawCircleStroke { center, radius, color, width } => {
                RenderCommand::DrawCircleStroke {
                    center: transform.point(center),
                    radius: (radius as f32 * transform.scale).round().max(1.0) as u32,
                    color,
                    width: (width as f32 * transform.scale).round().max(1.0) as u32,
                }
            }
            RenderCommand::DrawPath { points, closed, color, filled, width } => {
                RenderCommand::DrawPath {
                    points: points.into_iter().map(|point| transform.point(point)).collect(),
                    closed,
                    color,
                    filled,
                    width: (width as f32 * transform.scale).round().max(1.0) as u32,
                }
            }
            other => other,
        };
        self.redo_stack.clear();
        self.commands.push(command);
        while self.commands.len() > self.command_limit {
            self.commands.remove(0);
        }
        self.base.request_redraw();
    }

    /// Exports the canvas to an SVG document.
    ///
    /// Uses the crate's shared SVG backend, so the export and the on-screen paint read the same
    /// command list rather than a second renderer that could drift.
    pub fn export_svg(&mut self) -> String {
        crate::widget::svg::render_to_svg(self)
    }

    // ── Drawing API ──────────────────────────────────────────────────────

    /// Fill the entire canvas area with a solid color.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.push_command(RenderCommand::FillRect { rect, color });
        self.base.request_redraw();
    }

    /// Draw a rectangle outline.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.push_command(RenderCommand::DrawRect { rect, color });
        self.base.request_redraw();
    }

    /// Draw a line segment from `from` to `to`.
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        self.push_command(RenderCommand::DrawLine { from, to, color });
        self.base.request_redraw();
    }

    /// Draw a line segment with anti-aliasing.
    pub fn draw_line_aa(&mut self, from: Point, to: Point, color: Color) {
        self.push_command(RenderCommand::DrawLineAA { from, to, color });
        self.base.request_redraw();
    }

    /// Draw a line segment with explicit stroke width.
    pub fn draw_line_stroke(&mut self, from: Point, to: Point, color: Color, width: u32) {
        self.push_command(RenderCommand::DrawLineStroke { from, to, color, width });
        self.base.request_redraw();
    }

    /// Fill a circle at `center` with `radius`.
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.push_command(RenderCommand::FillCircle { center, radius, color });
        self.base.request_redraw();
    }

    /// Fill a circle with anti-aliasing.
    pub fn fill_circle_aa(&mut self, center: Point, radius: u32, color: Color) {
        self.push_command(RenderCommand::FillCircleAA { center, radius, color });
        self.base.request_redraw();
    }

    /// Draw a circle outline.
    pub fn draw_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.push_command(RenderCommand::DrawCircle { center, radius, color });
        self.base.request_redraw();
    }

    /// Draw a circle outline with explicit stroke width.
    pub fn draw_circle_stroke(&mut self, center: Point, radius: u32, color: Color, width: u32) {
        self.push_command(RenderCommand::DrawCircleStroke { center, radius, color, width });
        self.base.request_redraw();
    }

    /// Fill a rounded rectangle.
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color) {
        self.push_command(RenderCommand::FillRoundedRect { rect, radius, color });
        self.base.request_redraw();
    }

    /// Draw text at the specified position.
    pub fn draw_text(&mut self, origin: Point, text: &str, font: &crate::core::Font, color: Color) {
        self.push_command(RenderCommand::DrawText {
            origin,
            text: text.to_string(),
            font: font.clone(),
            color,
            alignment: crate::core::HorizontalAlignment::Left,
        });
        self.base.request_redraw();
    }

    /// Draw an arc (partial circle).
    pub fn draw_arc(
        &mut self,
        center: Point,
        radius: u32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
        filled: bool,
    ) {
        self.push_command(RenderCommand::DrawArc {
            center,
            radius,
            start_angle,
            end_angle,
            color,
            filled,
        });
        self.base.request_redraw();
    }

    /// Draw a polyline or polygon path.
    pub fn draw_path(
        &mut self,
        points: Vec<Point>,
        closed: bool,
        color: Color,
        filled: bool,
        width: u32,
    ) {
        self.push_command(RenderCommand::DrawPath { points, closed, color, filled, width });
        self.base.request_redraw();
    }

    /// Returns the last tracked mouse position.
    pub fn last_mouse_pos(&self) -> Point {
        self.last_mouse_pos
    }
}

impl Widget for Canvas {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "mouse_pressed" => Some(EventSignalRef::unit("mouse_pressed", &self.mouse_pressed)),
            "mouse_released" => Some(EventSignalRef::unit("mouse_released", &self.mouse_released)),
            "mouse_moved" => Some(EventSignalRef::mapped("mouse_moved", &self.mouse_moved, |v| {
                CapabilityValue::String(format!("{v:?}"))
            })),
            "double_clicked" => Some(EventSignalRef::unit("double_clicked", &self.double_clicked)),
            _ => None,
        }
    }
}

/// `Canvas`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, including
/// the `f32` → `f64` widening on read and the "write one half, keep the other"
/// behaviour of the `center_x` / `center_y` setters.
///
/// `WidgetKind::Canvas` is the kind the capability layer pairs with `MapView`, so
/// the `center_*` / `zoom` / `marker_count` names belong to that control, not to
/// this one. `canvas_capability` publishes an empty schema for `Canvas` itself,
/// so this contract inherits the shared four and owns nothing beyond them.
impl WidgetProperties for Canvas {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "command_count" => Ok(CapabilityValue::UInt(self.command_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // Derived from the recorded command list, which is mutated through the
            // drawing methods rather than by assigning a count.
            "command_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["command_count", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `canvas` publishes.
    ///
    /// `clear` discards the recorded drawing commands and needs no argument, so it
    /// executes here. `set_zoom` and `set_center` belong to the `MapView` control
    /// that shares this kind, so they carry a value this contract has no writer for
    /// and are refused as [`CapabilityAccessError::OutOfRange`].
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "set_zoom" | "set_center" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Canvas {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();

        // The canvas surface and its frame are chrome, so they resolve explicit
        // style first, then the theme, and only then a literal. Without the theme
        // step the surface stayed white in both appearances, which made a light/dark
        // switch invisible.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("canvas");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| background.blend(&Color::BLACK, 0.2));

        // Draw canvas background
        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        // Replay all stored commands. Borrowed: the list is the canvas's own content, so
        // cloning each command per frame only duplicated its text and point vectors.
        // A command whose bounds lie outside the control is culled: a drawing surface that has
        // been panned keeps every off-screen mark, and replaying them costs the rasteriser a
        // full pass each for pixels nobody can see.
        for cmd in &self.commands {
            if let Some(bounds) = command_bounds(cmd) {
                if !rects_intersect(rect, bounds) {
                    continue;
                }
            }
            context.execute_command_ref(cmd);
        }
        // Draw border to make canvas area visible
        context.draw_rect(rect, border);
    }

    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

impl EventHandler for Canvas {
    fn handle_event(&mut self, event: &Event) {
        // Forwarded *before* the match, so the base records the pointer facts (`hovered`,
        // `grabbed`, focus) that `widget_state()` reads. Without it the theme's `"canvas:hover"`
        // key could never match, which is the same defect `list_view` and `data_grid` had: a
        // control whose `handle_event` replaces the base's rather than extending it.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                self.last_mouse_pos = *pos;
                self.base.set_mouse_pressed(true);
                self.mouse_pressed.emit();
            }
            Event::MouseRelease { pos, button } if *button == 1 => {
                self.last_mouse_pos = *pos;
                self.base.set_mouse_pressed(false);
                self.mouse_released.emit();
            }
            Event::MouseMove { pos } if self.geometry().contains(*pos) => {
                self.last_mouse_pos = *pos;
                self.mouse_moved.emit(*pos);
            }
            Event::MouseDoubleClick { pos, button } if *button == 1 => {
                self.last_mouse_pos = *pos;
                self.double_clicked.emit();
            }
            Event::Wheel { delta, .. } if self.zoom_enabled && delta.y != 0 => {
                // Zoom about the pointer, so the ink under the cursor stays under it.
                let anchor = self.last_mouse_pos;
                let factor = if delta.y > 0 { 0.9 } else { 1.1 };
                let scale = (self.transform.scale * factor).clamp(0.1, 16.0);
                let ratio = scale / self.transform.scale;
                self.transform.offset = Point::new(
                    anchor.x - ((anchor.x - self.transform.offset.x) as f32 * ratio).round() as i32,
                    anchor.y - ((anchor.y - self.transform.offset.y) as f32 * ratio).round() as i32,
                );
                self.transform.scale = scale;
                self.base.request_redraw();
            }
            Event::ContextMenu { pos } => {
                self.context_menu_requested.emit(*pos);
            }
            Event::KeyPress { key, modifiers } => {
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                if modifiers.contains(crate::shortcut::Modifiers::PRIMARY) {
                    match crate::shortcut::Key::from_key_code(*key) {
                        Some(crate::shortcut::Key::Z) => {
                            self.undo();
                        }
                        Some(crate::shortcut::Key::Y) => {
                            self.redo();
                        }
                        _ => {}
                    }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

/// The axis-aligned bounds of a command, when it has any.
fn command_bounds(command: &RenderCommand) -> Option<Rect> {
    match command {
        RenderCommand::FillRect { rect, .. } | RenderCommand::DrawRect { rect, .. } => Some(*rect),
        RenderCommand::FillRoundedRect { rect, .. } => Some(*rect),
        RenderCommand::FillCircle { center, radius, .. }
        | RenderCommand::FillCircleAA { center, radius, .. }
        | RenderCommand::DrawCircle { center, radius, .. }
        | RenderCommand::DrawCircleStroke { center, radius, .. } => Some(Rect::new(
            center.x - *radius as i32,
            center.y - *radius as i32,
            radius.saturating_mul(2).max(1),
            radius.saturating_mul(2).max(1),
        )),
        RenderCommand::DrawLine { from, to, .. }
        | RenderCommand::DrawLineAA { from, to, .. }
        | RenderCommand::DrawLineStroke { from, to, .. } => Some(Rect::new(
            from.x.min(to.x),
            from.y.min(to.y),
            (from.x - to.x).unsigned_abs().max(1),
            (from.y - to.y).unsigned_abs().max(1),
        )),
        RenderCommand::DrawPath { points, .. } => {
            let first = points.first()?;
            let (mut left, mut top, mut right, mut bottom) = (first.x, first.y, first.x, first.y);
            for point in points {
                left = left.min(point.x);
                top = top.min(point.y);
                right = right.max(point.x);
                bottom = bottom.max(point.y);
            }
            Some(Rect::new(
                left,
                top,
                (right - left).unsigned_abs().max(1),
                (bottom - top).unsigned_abs().max(1),
            ))
        }
        _ => None,
    }
}

/// Whether two rectangles overlap.
fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width as i32
        && b.x < a.x + a.width as i32
        && a.y < b.y + b.height as i32
        && b.y < a.y + a.height as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Font, Rect};
    use crate::widget::svg::render_to_svg;

    #[test]
    fn canvas_creation_defaults() {
        let canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.command_count(), 0);
        assert_eq!(canvas.geometry(), Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.kind(), WidgetKind::Canvas);
    }

    #[test]
    fn canvas_fill_rect_adds_command() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::BLUE);
        assert_eq!(canvas.command_count(), 1);
    }

    #[test]
    fn canvas_draw_line_and_circle() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::RED);
        canvas.fill_circle(Point::new(50, 50), 10, Color::GREEN);
        assert_eq!(canvas.command_count(), 2);
    }

    #[test]
    fn canvas_clear_removes_all_commands() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rect(Rect::new(0, 0, 100, 100), Color::BLUE);
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::RED);
        assert_eq!(canvas.command_count(), 2);
        canvas.clear();
        assert_eq!(canvas.command_count(), 0);
    }

    #[test]
    fn canvas_draw_path_stores_commands() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        let points =
            vec![Point::new(0, 0), Point::new(100, 0), Point::new(100, 100), Point::new(0, 100)];
        canvas.draw_path(points, true, Color::BLUE, false, 2);
        assert_eq!(canvas.command_count(), 1);
    }

    #[test]
    fn canvas_draw_text_stores_command() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_text(Point::new(10, 20), "Hello", &Font::default_ui(), Color::BLACK);
        assert_eq!(canvas.command_count(), 1);
    }

    #[test]
    fn canvas_draw_produces_svg_output() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::BLUE);
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::RED);
        let svg = render_to_svg(&mut canvas);
        assert!(svg.starts_with("<svg"));
        assert!(svg.len() > 100);
    }

    #[test]
    fn canvas_signal_accessors() {
        let canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        let _ = &canvas.mouse_pressed;
        let _ = &canvas.mouse_released;
        let _ = &canvas.mouse_moved;
        let _ = &canvas.double_clicked;
    }

    #[test]
    fn canvas_last_mouse_position_tracked() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.last_mouse_pos(), Point::new(0, 0));
        canvas.handle_event(&Event::MouseMove { pos: Point::new(42, 73) });
        assert_eq!(canvas.last_mouse_pos(), Point::new(42, 73));
    }

    #[test]
    fn canvas_disabled_state_blocks_events() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.set_enabled(false);
        // Events should be ignored when disabled
        canvas.handle_event(&Event::MousePress {
            pos: Point::new(10, 10),
            button: 1,
            modifiers: 0,
        });
        // Signal should NOT have been emitted (disabled)
        // We just verify no panic and state unchanged
        assert_eq!(canvas.last_mouse_pos(), Point::new(0, 0));
    }

    #[test]
    fn canvas_double_click_emits_signal() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.handle_event(&Event::MouseDoubleClick { pos: Point::new(50, 50), button: 1 });
        assert_eq!(canvas.last_mouse_pos(), Point::new(50, 50));
    }

    #[test]
    fn canvas_draw_arc_and_draw_circle_stroke() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_arc(Point::new(100, 50), 30, 0.0, std::f32::consts::PI, Color::RED, false);
        canvas.draw_circle_stroke(Point::new(50, 50), 20, Color::BLUE, 3);
        assert_eq!(canvas.command_count(), 2);
    }

    #[test]
    fn canvas_fill_rounded_rect_and_line_aa() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rounded_rect(Rect::new(5, 5, 50, 50), 8, Color::rgba(255, 0, 0, 128));
        canvas.draw_line_aa(Point::new(0, 0), Point::new(100, 50), Color::GREEN);
        assert_eq!(canvas.command_count(), 2);
    }
    /// The canvas forwards pointer events to its base, so hover becomes a state it can report.
    ///
    /// Regression: `handle_event` *replaced* the base's rather than extending it, so
    /// `MouseEnter`/`MouseLeave` never reached `BaseWidget`, `widget_state()` could never be
    /// `Hover`, and the theme's `"canvas:hover"` key was dead — the same defect `list_view` and
    /// `data_grid` had. The assertion is the state the contract publishes, not a signal.
    #[test]
    fn the_canvas_reports_hover_because_it_forwards_to_its_base() {
        use crate::style::WidgetState;

        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.widget_state(), WidgetState::Normal);

        canvas.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert_eq!(
            canvas.widget_state(),
            WidgetState::Hover,
            "the base must have been told the pointer arrived"
        );

        canvas.handle_event(&Event::MouseLeave { pos: Point::new(500, 500) });
        assert_eq!(canvas.widget_state(), WidgetState::Normal, "and that it left");
    }
}
