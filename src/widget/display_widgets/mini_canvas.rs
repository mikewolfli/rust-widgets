// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MiniCanvas widget — simplified drawing surface for mini builds (BLUE13 R2.11).
use crate::compat::Vec;
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::signal::GenericSignal;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Simplified canvas for drawing basic shapes.
pub struct MiniCanvas {
    base: BaseWidget,
    commands: Vec<RenderCommand>,
    last_mouse_pos: Point,
    /// Emitted when a click-like interaction (press then release) is detected.
    pub clicked: GenericSignal,
    /// Emitted when the mouse button is pressed over the canvas.
    pub mouse_pressed: GenericSignal,
    /// Emitted when the mouse button is released over the canvas.
    pub mouse_released: GenericSignal,
}

impl MiniCanvas {
    /// Creates a new MiniCanvas with the given geometry.
    pub fn new(rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MiniCanvas, rect, "MiniCanvas"),
            commands: Vec::new(),
            last_mouse_pos: Point::new(0, 0),
            clicked: GenericSignal::new(),
            mouse_pressed: GenericSignal::new(),
            mouse_released: GenericSignal::new(),
        }
    }

    /// Add a fill rectangle command.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.commands.push(RenderCommand::FillRect { rect, color });
    }

    /// Add a draw rectangle outline command.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.commands.push(RenderCommand::DrawRect { rect, color });
    }

    /// Add a draw line command.
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        self.commands.push(RenderCommand::DrawLine { from, to, color });
    }

    /// Add a fill circle command.
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.commands.push(RenderCommand::FillCircle { center, radius, color });
    }

    /// Clear all commands.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Clear commands and add a fill rect for the background.
    pub fn clear_with_color(&mut self, color: Color) {
        self.commands.clear();
        let rect = self.base.geometry();
        self.commands.push(RenderCommand::FillRect { rect, color });
    }

    /// Get stored commands.
    pub fn commands(&self) -> &[RenderCommand] {
        &self.commands
    }
}

impl Widget for MiniCanvas {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(200, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `MiniCanvas`'s property contract.
///
/// `MiniCanvas` carries only its retained draw command list, and the legacy
/// dispatch served no property for it (its schema table `MINI_CANVAS_PROPERTIES`
/// is empty), so the contract publishes the shared four and nothing else. It owns
/// no derived count worth exposing: `commands()` is the retained command list,
/// which callers read directly rather than through reflection.
impl WidgetProperties for MiniCanvas {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        base_property_get(self, name)
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        base_property_set(self, name, value)
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `mini_canvas` publishes.
    ///
    /// `clear` is the one name here that is a genuine zero-argument action: it drops
    /// every retained draw command, which is exactly what
    /// [`MiniCanvas::clear`] does.
    ///
    /// The four drawing names are a different case. Each takes geometry and a colour,
    /// so `fill_rect()` / `draw_rect()` / `draw_line()` / `fill_circle()` append a
    /// shape — state a caller has to supply — and a command carries no argument that
    /// could describe the rectangle or the endpoints. They are therefore refused as
    /// [`CapabilityAccessError::OutOfRange`]: the names are right and the invocation
    /// needs the inherent API, which is not `UnknownCommand`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "fill_rect" | "draw_rect" | "draw_line" | "fill_circle" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for MiniCanvas {
    /// Pointer events reach a canvas only while it is enabled and only when they land on it.
    ///
    /// Neither guard was here: a disabled canvas still emitted `mouse_pressed`,
    /// `mouse_released` and `clicked` for a press anywhere in the window, so a host that
    /// disables the widget to take it out of play kept receiving interaction callbacks from it.
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, .. } => {
                // A press outside is not an interaction; without this check the position was
                // recorded and the event reported for any pointer-down the host routed here.
                if !self.geometry().contains_point(*pos) {
                    return;
                }
                self.last_mouse_pos = *pos;
                self.mouse_pressed.emit();
            }
            Event::MouseRelease { pos, .. } => {
                if !self.geometry().contains_point(*pos) {
                    return;
                }
                self.last_mouse_pos = *pos;
                self.mouse_released.emit();
                self.clicked.emit();
            }
            _ => {}
        }
    }
}

impl Draw for MiniCanvas {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // The canvas surface resolves the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. It used to fall back to a fixed white, so a
        // light/dark switch left the sheet unchanged — the rendering census reported the control as
        // theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.style().clone();
        // `crate::theme` exists only in a `full_widgets` build (see `src/theme/mod.rs` and
        // principle #47). A `mini`/`embedded` build has no colour model at all, so the theme
        // read has to be conditioned on the module being there — naming it unconditionally
        // is what broke those two profiles.
        //
        // The fallback is *not* a fabricated theme: it is the same explicit ladder the
        // control documents (explicit style -> resolved theme style -> literal), with the
        // middle rung absent because the profile has no theme. A caller in that profile
        // still gets the explicit style when it set one.
        #[cfg(full_widgets)]
        let theme = crate::style::resolved_theme_style("mini_canvas");
        #[cfg(not(full_widgets))]
        let theme: Option<crate::style::WidgetStyle> = None;
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        #[cfg(full_widgets)]
        let (window_fill, foreground, secondary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => {
                    (active.colors.background, active.colors.foreground, active.colors.secondary)
                }
                None => (Color::rgb(240, 240, 240), Color::BLACK, Color::rgb(158, 158, 158)),
            }
        };
        // No theme module: the literals are the only rung available, and they are the same
        // ones the `None` arm above uses so the two profiles render alike.
        #[cfg(not(full_widgets))]
        let (window_fill, foreground, secondary) =
            (Color::rgb(240, 240, 240), Color::BLACK, Color::rgb(158, 158, 158));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `mini_canvas` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and the active theme writes the window fill into `style.background_color`. A
        // sheet painted in that colour would be byte-identical to the frame behind it, which is
        // exactly the "painted, but invisible" defect the census reports as `ink = 0`. A resolved
        // surface equal to the window fill is therefore re-derived a visible step away from it,
        // while a colour the caller set still wins.
        let sheet = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        context.fill_rect(rect, sheet);

        // A page edge, so an empty canvas still reads as a bounded sheet rather than as bare
        // surface. Drawn before the commands, which are the caller's own shapes and paint over it.
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != sheet)
            .unwrap_or_else(|| sheet.blend(&secondary, 0.45));
        context.draw_rect(rect, border);

        // Replay all stored commands.
        for cmd in &self.commands {
            context.execute_command(cmd.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    #[test]
    fn mini_canvas_creation() {
        let canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        assert_eq!(canvas.kind(), WidgetKind::MiniCanvas);
        assert_eq!(canvas.geometry(), Rect::new(0, 0, 200, 200));
        assert!(canvas.commands().is_empty());
    }

    #[test]
    fn mini_canvas_fill_rect() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        assert!(canvas.commands().is_empty());

        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::RED);
        assert_eq!(canvas.commands().len(), 1);

        canvas.fill_rect(Rect::new(70, 70, 30, 30), Color::BLUE);
        assert_eq!(canvas.commands().len(), 2);
    }

    #[test]
    fn mini_canvas_draw_line() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::BLACK);
        assert_eq!(canvas.commands().len(), 1);
        match &canvas.commands()[0] {
            RenderCommand::DrawLine { from, to, color } => {
                assert_eq!(*from, Point::new(0, 0));
                assert_eq!(*to, Point::new(100, 100));
                assert_eq!(*color, Color::BLACK);
            }
            _ => panic!("Expected DrawLine command"),
        }
    }

    #[test]
    fn mini_canvas_clear() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::RED);
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::BLACK);
        assert_eq!(canvas.commands().len(), 2);

        canvas.clear();
        assert!(canvas.commands().is_empty());
    }

    #[test]
    fn mini_canvas_draw_no_panic() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));

        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        canvas.draw(&mut context);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn mini_canvas_clear_with_color() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::RED);
        assert_eq!(canvas.commands().len(), 1);

        canvas.clear_with_color(Color::rgb(200, 200, 200));
        assert_eq!(canvas.commands().len(), 1);
        match &canvas.commands()[0] {
            RenderCommand::FillRect { color, .. } => {
                assert_eq!(*color, Color::rgb(200, 200, 200));
            }
            _ => panic!("Expected FillRect command"),
        }
    }

    #[test]
    fn mini_canvas_fill_circle() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.fill_circle(Point::new(100, 100), 30, Color::GREEN);
        assert_eq!(canvas.commands().len(), 1);
        match &canvas.commands()[0] {
            RenderCommand::FillCircle { center, radius, color } => {
                assert_eq!(*center, Point::new(100, 100));
                assert_eq!(*radius, 30);
                assert_eq!(*color, Color::GREEN);
            }
            _ => panic!("Expected FillCircle command"),
        }
    }
    /// A disabled canvas reports nothing, and a press outside it is not an interaction.
    ///
    /// Neither guard existed: the handler emitted `mouse_pressed`, `mouse_released` and `clicked`
    /// while disabled, and did so for a press anywhere in the window because the position was
    /// never tested against the geometry.
    #[test]
    fn mini_canvas_ignores_input_when_disabled_and_when_outside() {
        use crate::event::Event;
        use crate::widget::Widget;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let geometry = Rect::new(0, 0, 100, 100);
        let build = || {
            let canvas = MiniCanvas::new(geometry);
            let hits = Arc::new(AtomicUsize::new(0));
            let sink = Arc::clone(&hits);
            canvas.mouse_pressed.connect(move || {
                sink.fetch_add(1, Ordering::SeqCst);
            });
            let clicks = Arc::new(AtomicUsize::new(0));
            let sink = Arc::clone(&clicks);
            canvas.clicked.connect(move || {
                sink.fetch_add(1, Ordering::SeqCst);
            });
            (canvas, hits, clicks)
        };

        // Enabled and inside: both signals fire.
        let (mut canvas, hits, clicks) = build();
        canvas.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        canvas.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(clicks.load(Ordering::SeqCst), 1);

        // Disabled: nothing fires.
        let (mut canvas, hits, clicks) = build();
        canvas.set_enabled(false);
        canvas.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        canvas.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert_eq!(hits.load(Ordering::SeqCst), 0, "a disabled canvas must stay out of play");
        assert_eq!(clicks.load(Ordering::SeqCst), 0);

        // Enabled but outside: nothing fires.
        let (mut canvas, hits, clicks) = build();
        let outside = Point::new(5000, 5000);
        canvas.handle_event(&Event::MousePress { pos: outside, button: 1 });
        canvas.handle_event(&Event::MouseRelease { pos: outside, button: 1 });
        assert_eq!(hits.load(Ordering::SeqCst), 0, "a press outside is not an interaction");
        assert_eq!(clicks.load(Ordering::SeqCst), 0);
    }
}
