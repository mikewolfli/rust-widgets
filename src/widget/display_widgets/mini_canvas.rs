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

/// The default command-history bound.
///
/// A canvas is a retained display list, so an unbounded list is a memory leak in a control that
/// draws every frame. Ten thousand commands is several full screens of shapes; a caller that
/// legitimately needs more can raise the bound with [`MiniCanvas::set_max_commands`].
const DEFAULT_MAX_COMMANDS: usize = 10_000;

/// Simplified canvas for drawing basic shapes.
pub struct MiniCanvas {
    base: BaseWidget,
    commands: Vec<RenderCommand>,
    /// The largest number of commands retained; the oldest is dropped when a push would exceed it.
    max_commands: usize,
    /// Whether the retained commands have changed since the last captured frame.
    dirty: bool,
    /// Increases whenever the retained commands or their meaning change.
    change_counter: u64,
    /// Whether a frame has been replayed and its presented pixels still need capturing.
    ///
    /// A paint backend presents a frame on `end_frame`, so the pixels of the frame the canvas
    /// just replayed are not readable until the *next* paint begins. The capture is therefore
    /// deferred by one paint rather than reading a half-built back buffer.
    capture_pending: bool,
    /// The rectangle the pending capture belongs to, so a moved canvas does not capture the
    /// wrong region.
    pending_rect: Rect,
    /// Whether the command list should be dropped as soon as a capture succeeds.
    flush_pending: bool,
    /// The last painted frame region, `cache_width * cache_height * 4` RGBA bytes.
    cache: Vec<u8>,
    cache_width: u32,
    cache_height: u32,
    /// The `change_counter` value the cached pixels correspond to.
    cache_counter: u64,
    /// Whether the command list is dropped after a frame has been replayed and captured.
    auto_flush: bool,
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
            max_commands: DEFAULT_MAX_COMMANDS,
            dirty: false,
            change_counter: 0,
            capture_pending: false,
            pending_rect: Rect::new(0, 0, 0, 0),
            flush_pending: false,
            cache: Vec::new(),
            cache_width: 0,
            cache_height: 0,
            // A sentinel that cannot equal a real counter, so an empty cache is never mistaken
            // for a current one.
            cache_counter: u64::MAX,
            auto_flush: false,
            clicked: GenericSignal::new(),
            mouse_pressed: GenericSignal::new(),
            mouse_released: GenericSignal::new(),
        }
    }

    /// Appends one command, dropping the oldest when the bounded history is full.
    ///
    /// # The bound is a history, not a silent loss
    ///
    /// The default bound is [`DEFAULT_MAX_COMMANDS`] entries. When a push would exceed
    /// [`MiniCanvas::max_commands`], the **oldest** command leaves the list first, so the most
    /// recently submitted shapes are the ones that survive. This is the documented contract of a
    /// bounded history: a caller that relies on an ancient command still being present must either
    /// raise the bound or submit it again. Commands are never dropped without that rule applying.
    fn push_command(&mut self, command: RenderCommand) {
        if self.commands.len() >= self.max_commands {
            self.commands.remove(0);
        }
        self.commands.push(command);
        self.change_counter = self.change_counter.wrapping_add(1);
        self.dirty = true;
        // The previously presented frame no longer describes this state, and a pending flush
        // would drop the new command before it is ever painted.
        self.capture_pending = false;
        self.flush_pending = false;
        self.base.request_redraw();
    }

    /// Add a fill rectangle command.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.push_command(RenderCommand::FillRect { rect, color });
    }

    /// Add a draw rectangle outline command.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.push_command(RenderCommand::DrawRect { rect, color });
    }

    /// Add a draw line command.
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        self.push_command(RenderCommand::DrawLine { from, to, color });
    }

    /// Add a fill circle command.
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.push_command(RenderCommand::FillCircle { center, radius, color });
    }

    /// The largest number of commands the canvas retains. Defaults to [`DEFAULT_MAX_COMMANDS`].
    pub fn max_commands(&self) -> usize {
        self.max_commands
    }

    /// Sets the command-history bound.
    ///
    /// A value below `1` is raised to `1`, because a canvas that retains nothing could not draw a
    /// shape at all. Shrinking the bound below the current length drops the oldest commands
    /// immediately, so the list is never left larger than its own declared bound. Growing the
    /// bound changes nothing about the commands already retained.
    pub fn set_max_commands(&mut self, max: usize) {
        let max = max.max(1);
        if self.max_commands == max {
            return;
        }
        self.max_commands = max;
        if self.commands.len() > max {
            let excess = self.commands.len() - max;
            self.commands.drain(0..excess);
            self.change_counter = self.change_counter.wrapping_add(1);
            self.dirty = true;
            self.capture_pending = false;
            self.flush_pending = false;
            self.base.request_redraw();
        }
    }

    /// Clear all commands.
    ///
    /// This is a change to the picture, so the cached frame is invalidated and the next paint
    /// replays the (now empty) list.
    pub fn clear(&mut self) {
        if self.commands.is_empty() && self.cache.is_empty() {
            return;
        }
        self.commands.clear();
        self.change_counter = self.change_counter.wrapping_add(1);
        self.dirty = true;
        self.capture_pending = false;
        self.flush_pending = false;
        self.base.request_redraw();
    }

    /// Drops the command history without changing the painted picture.
    ///
    /// Called after a frame has replayed the list, or by a caller that has finished submitting a
    /// scene: the pixels already captured in the canvas's frame cache stay on screen, while the
    /// commands themselves no longer occupy memory or replay time. When no frame has been captured
    /// yet, the flush is deferred to the next paint that can capture the presented frame — a flush
    /// is never allowed to lose the picture.
    pub fn flush(&mut self) {
        if self.commands.is_empty() {
            return;
        }
        let cache_current = !self.dirty
            && !self.cache.is_empty()
            && self.cache_counter == self.change_counter
            && !self.capture_pending;
        if cache_current {
            self.commands.clear();
            self.flush_pending = false;
        } else {
            self.flush_pending = true;
        }
    }

    /// Whether the command list is dropped automatically after a painted, captured frame.
    pub fn auto_flush(&self) -> bool {
        self.auto_flush
    }

    /// Enables or disables automatic flushing after a captured frame.
    ///
    /// The default is `false`, which keeps the historical behaviour: the command list is retained
    /// and replayed every frame. With the option on, a frame that both replays the list and captures
    /// the resulting pixels drops the list afterwards, so the next paint blits the cache instead of
    /// replaying. A backend that cannot supply pixels never triggers the flush, because dropping the
    /// list there would lose the picture.
    pub fn set_auto_flush(&mut self, on: bool) {
        if self.auto_flush == on {
            return;
        }
        self.auto_flush = on;
    }

    /// Clear commands and add a fill rect for the background.
    pub fn clear_with_color(&mut self, color: Color) {
        self.commands.clear();
        let rect = self.base.geometry();
        self.commands.push(RenderCommand::FillRect { rect, color });
        self.change_counter = self.change_counter.wrapping_add(1);
        self.dirty = true;
        self.capture_pending = false;
        self.flush_pending = false;
        self.base.request_redraw();
    }

    /// Get stored commands.
    pub fn commands(&self) -> &[RenderCommand] {
        &self.commands
    }

    /// Captures the widget's region from the last **presented** frame into the cache.
    ///
    /// The capture is the frame as presented, so it is only used when the region is wholly opaque:
    /// an opaque sheet fully describes its own pixels and nothing behind it shows through. A host
    /// that repaints isolated subtrees should use its own damage tracking rather than relying on
    /// this cache for translucency.
    ///
    /// A paint backend presents on `end_frame`, so this is called at the start of the next paint,
    /// when the previous frame is readable. Returns `false` when the backend cannot supply pixels,
    /// when the device scale is not 1:1 (the cache stores physical bytes and a blit is not
    /// resampled), when the region is not wholly opaque (a translucent sheet does not describe the
    /// window behind it), or when the widget is not wholly inside the surface. In every failure
    /// case the cache is emptied so a stale frame is never blitted in place of a real one.
    fn capture_cache(&mut self, context: &mut RenderContext<'_>, rect: Rect) -> bool {
        self.cache.clear();
        if (context.dpi_scale() - 1.0).abs() > f32::EPSILON {
            return false;
        }
        let surface = context.size();
        let (offset_x, offset_y) = context.offset();
        let left = rect.x + offset_x;
        let top = rect.y + offset_y;
        if left < 0 || top < 0 {
            return false;
        }
        let left = left as u32;
        let top = top as u32;
        if left.saturating_add(rect.width) > surface.width
            || top.saturating_add(rect.height) > surface.height
        {
            return false;
        }
        let frame = context.backend().frame_rgba();
        let stride = surface.width as usize * 4;
        let expected = stride * surface.height as usize;
        if frame.len() < expected || rect.width == 0 || rect.height == 0 {
            return false;
        }
        let row_bytes = rect.width as usize * 4;
        self.cache.reserve(rect.height as usize * row_bytes);
        for row in 0..rect.height {
            let start = (top + row) as usize * stride + left as usize * 4;
            let pixels = &frame[start..start + row_bytes];
            // An opaque sheet covers every pixel it paints, so a fully opaque region is the proof
            // that this really is a presented widget frame rather than an untouched buffer. A
            // fresh surface therefore refuses the capture, and the command list stays authoritative.
            if pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 255) {
                self.cache.clear();
                return false;
            }
            self.cache.extend_from_slice(pixels);
        }
        self.cache_width = rect.width;
        self.cache_height = rect.height;
        self.cache_counter = self.change_counter;
        self.dirty = false;
        true
    }

    /// Captures the pending frame, then retires the command list if a flush was requested.
    ///
    /// Nothing happens until a frame has been replayed (`capture_pending`) and the canvas still
    /// occupies the rectangle that frame was painted into. A failure leaves the pending state in
    /// place so the next paint retries — a recording backend that supplies no pixels simply keeps
    /// replaying the commands, which is the historical behaviour.
    fn try_capture_pending(&mut self, context: &mut RenderContext<'_>, rect: Rect) {
        if !self.capture_pending {
            return;
        }
        if self.pending_rect != rect {
            // The canvas moved, so the previous frame describes a different region.
            self.capture_pending = false;
            return;
        }
        if self.capture_cache(context, rect) {
            self.capture_pending = false;
            if self.flush_pending {
                self.commands.clear();
                self.flush_pending = false;
            }
        }
    }
}

impl Widget for MiniCanvas {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(200, 200)
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
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "clicked" => Some(EventSignalRef::unit("clicked", &self.clicked)),
            "mouse_pressed" => Some(EventSignalRef::unit("mouse_pressed", &self.mouse_pressed)),
            "mouse_released" => Some(EventSignalRef::unit("mouse_released", &self.mouse_released)),
            _ => None,
        }
    }
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
                // A press outside is not an interaction; without this check the event was
                // reported for any pointer-down the host routed here.
                if !self.geometry().contains_point(*pos) {
                    return;
                }
                self.mouse_pressed.emit();
            }
            Event::MouseRelease { pos, .. } => {
                if !self.geometry().contains_point(*pos) {
                    return;
                }
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

        // Read the previously presented frame before this paint dirties the back buffer. A capture
        // that was pending from the last replay lands here, and may retire the command list.
        self.try_capture_pending(context, rect);

        // The canvas surface resolves the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. It used to fall back to a fixed white, so a
        // light/dark switch left the sheet unchanged — the rendering census reported the control as
        // theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.style();
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
        let theme = crate::style::resolved_theme_style_shared("mini_canvas");
        #[cfg(not(full_widgets))]
        let theme: Option<crate::style::WidgetStyle> = None;
        // One shared palette read for the three colours, copied out as values so no guard
        // survives the rest of the draw. This is the same single-snapshot form `progress_bar`
        // uses, rather than a second manager-lock acquisition on the frame path.
        #[cfg(full_widgets)]
        let (window_fill, foreground, secondary) = match crate::style::palette_shared() {
            Some(colors) => (colors.background, colors.foreground, colors.secondary),
            None => (Color::rgb(240, 240, 240), Color::BLACK, Color::rgb(158, 158, 158)),
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
        context.face_with_gradient(
            rect,
            sheet,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // A page edge, so an empty canvas still reads as a bounded sheet rather than as bare
        // surface. Drawn before the commands, which are the caller's own shapes and paint over it.
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != sheet)
            .unwrap_or_else(|| sheet.blend(&secondary, 0.45));
        context.draw_rect(rect, border);

        // Replay all stored commands without cloning the display list, unless a captured frame
        // already describes exactly this state. The cache is only trusted when the surface is
        // opaque (otherwise the pixels under the sheet matter), the device scale is 1:1 (a blit is
        // not resampled), the size still matches, and no command has changed since capture.
        let cache_current = sheet.a == 255
            && !self.dirty
            && !self.capture_pending
            && !self.cache.is_empty()
            && self.cache_counter == self.change_counter
            && self.cache_width == rect.width
            && self.cache_height == rect.height;
        if cache_current {
            context.draw_image(rect.x, rect.y, self.cache_width, self.cache_height, &self.cache);
            return;
        }
        for cmd in &self.commands {
            context.execute_command_ref(cmd);
        }
        self.dirty = false;
        if sheet.a != 255 {
            // A translucent sheet lets the frame behind the canvas show through, so a cached
            // blit would freeze that backdrop. Keep the command list authoritative instead.
            self.capture_pending = false;
            self.cache.clear();
            return;
        }
        // The pixels of this frame become readable at the next `end_frame`; remember that they
        // are owed, and retire the list then if a flush was requested (explicitly or by policy).
        self.capture_pending = true;
        self.pending_rect = rect;
        if self.auto_flush {
            self.flush_pending = true;
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
        canvas.handle_event(&Event::MousePress {
            pos: Point::new(10, 10),
            button: 1,
            modifiers: 0,
        });
        canvas.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(clicks.load(Ordering::SeqCst), 1);

        // Disabled: nothing fires.
        let (mut canvas, hits, clicks) = build();
        canvas.set_enabled(false);
        canvas.handle_event(&Event::MousePress {
            pos: Point::new(10, 10),
            button: 1,
            modifiers: 0,
        });
        canvas.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert_eq!(hits.load(Ordering::SeqCst), 0, "a disabled canvas must stay out of play");
        assert_eq!(clicks.load(Ordering::SeqCst), 0);

        // Enabled but outside: nothing fires.
        let (mut canvas, hits, clicks) = build();
        let outside = Point::new(5000, 5000);
        canvas.handle_event(&Event::MousePress { pos: outside, button: 1, modifiers: 0 });
        canvas.handle_event(&Event::MouseRelease { pos: outside, button: 1 });
        assert_eq!(hits.load(Ordering::SeqCst), 0, "a press outside is not an interaction");
        assert_eq!(clicks.load(Ordering::SeqCst), 0);
    }

    /// Renders the canvas into the given surface and returns the presented pixels.
    ///
    /// The same backend is reused across a test's renders on purpose: a paint backend presents a
    /// frame on `end_frame`, and the canvas captures that presented frame at the start of the next
    /// paint. Reusing the surface is what makes the cache observable.
    fn render_canvas(backend: &mut SoftwarePaintBackend, canvas: &mut MiniCanvas) -> Vec<u8> {
        backend.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(backend);
            canvas.draw(&mut context);
        }
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// The command list is a bounded history: the oldest command leaves when the cap is reached.
    #[test]
    fn mini_canvas_command_cap_drops_the_oldest_command() {
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.set_max_commands(2);
        assert_eq!(canvas.max_commands(), 2, "the setter stores the bound it was given");

        canvas.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
        canvas.fill_rect(Rect::new(0, 0, 10, 10), Color::GREEN);
        canvas.fill_rect(Rect::new(0, 0, 10, 10), Color::BLUE);

        assert_eq!(canvas.commands().len(), 2, "the bound is never exceeded");
        match &canvas.commands()[0] {
            RenderCommand::FillRect { color, .. } => {
                assert_eq!(*color, Color::GREEN, "the oldest command is the one dropped")
            }
            other => panic!("expected the second FillRect, got {other:?}"),
        }
        match &canvas.commands()[1] {
            RenderCommand::FillRect { color, .. } => assert_eq!(*color, Color::BLUE),
            other => panic!("expected the newest FillRect, got {other:?}"),
        }
    }

    /// A flush drops the history after a frame has been captured, and the captured pixels remain.
    #[test]
    fn mini_canvas_flush_keeps_the_captured_frame() {
        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.fill_rect(Rect::new(20, 20, 100, 60), Color::RED);

        let painted = render_canvas(&mut backend, &mut canvas);
        assert_eq!(canvas.commands().len(), 1, "the frame replayed the retained command");

        canvas.flush();
        assert_eq!(
            canvas.commands().len(),
            1,
            "a flush is deferred until the presented frame can be captured"
        );

        let replayed = render_canvas(&mut backend, &mut canvas);
        assert!(canvas.commands().is_empty(), "the capture takes the flush with it");
        assert_eq!(
            painted, replayed,
            "a paint after flush must blit the captured frame, not an empty list"
        );

        let cached = render_canvas(&mut backend, &mut canvas);
        assert_eq!(painted, cached, "the following paint replays the captured pixels");
    }

    /// With auto-flush on, a frame's history is dropped once its pixels have been captured.
    #[test]
    fn mini_canvas_auto_flush_drops_history_after_a_captured_frame() {
        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.set_auto_flush(true);
        assert!(canvas.auto_flush());
        canvas.fill_circle(Point::new(100, 100), 30, Color::GREEN);

        let painted = render_canvas(&mut backend, &mut canvas);
        assert_eq!(canvas.commands().len(), 1, "the first frame still needs the command");

        let replayed = render_canvas(&mut backend, &mut canvas);
        assert!(canvas.commands().is_empty(), "the captured frame retires the history");
        assert_eq!(
            painted, replayed,
            "the next paint must reproduce the cached frame byte for byte"
        );

        let cached = render_canvas(&mut backend, &mut canvas);
        assert_eq!(painted, cached, "the following paint replays the captured pixels");
    }

    /// Adding a command after a captured frame invalidates the cache and replays the new list.
    #[test]
    fn mini_canvas_new_command_invalidates_the_cached_frame() {
        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        let mut canvas = MiniCanvas::new(Rect::new(0, 0, 200, 200));
        canvas.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
        let before = render_canvas(&mut backend, &mut canvas);

        canvas.fill_rect(Rect::new(50, 50, 100, 100), Color::BLUE);
        let after = render_canvas(&mut backend, &mut canvas);

        assert_ne!(before, after, "a new command must change the painted pixels");
        assert_eq!(canvas.commands().len(), 2, "both commands are retained");
    }
}
