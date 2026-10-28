// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Window widget and platform integration.
use crate::compat::{String, ToString};
use crate::core::{Color, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
#[cfg(device_profile)]
use crate::theme::Material;
use crate::widget::capability::coercion::{expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Main application window.
pub struct Window {
    base: BaseWidget,
    title: String,
    title_bar_height: u32,
    close_button_size: u32,
    button_spacing: u32,
    /// The semantic material the window's client surface asks for.
    ///
    /// Only a device build has the theme module that defines [`Material`]; a stripped build
    /// paints its own background and has no material vocabulary to name, so the field and
    /// its accessors are absent rather than carrying a duplicate local enum.
    #[cfg(device_profile)]
    material: Material,
    // G-301: `min_size`/`max_size` are deliberately **not** stored here. `BaseWidget` owns the
    // single authoritative bound store, and `Window::set_min_size`/`set_max_size` forward into it
    // so the platform request and the layout hint reported by `Widget::hints` cannot drift apart.
    /// Whether the window is modal.
    ///
    /// Host-facing intent record only (G-302): the platform trait has no modality call and no
    /// library production code reads this field. [`Window::is_modal`] is the host's query path.
    /// The value is not a mirror of platform state, so there is no authoritative source for it
    /// to drift from.
    modal: bool,
    /// Whether the window asks to float above others.
    ///
    /// Host-facing intent record only (G-302), like [`Self::modal`]: no platform call and no
    /// in-library reader. [`Window::is_always_on_top`] is the query path.
    always_on_top: bool,
    /// Window opacity in `0.0..=1.0`.
    ///
    /// Host-facing intent record only (G-302): neither the draw path nor any other library code
    /// reads it. [`Window::opacity`] is the query path for a host that applies it.
    opacity: f32,
    /// Icon path requested through the platform, when one was set.
    ///
    /// The only field of the host-facing group with a platform write path, but still no
    /// in-library reader (G-302): [`Window::icon`] reports what was requested to the host. It is
    /// retained because deleting it would delete the public getter with it.
    icon: Option<String>,
    /// Whether the window paints its own chrome for a borderless window.
    ///
    /// Off by default: a decorated window is drawn by the window manager, and painting a
    /// second title bar inside it was the defect this control already records in `draw`.
    decorated: bool,
    /// Emitted when the window is closed.
    pub closed: GenericSignal,
}
impl Window {
    /// Creates a new window with title and geometry.
    pub fn new(title: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Window, geometry, "Window"),
            title,
            title_bar_height: 32,
            close_button_size: 14,
            button_spacing: 40,
            #[cfg(device_profile)]
            material: Material::None,
            modal: false,
            always_on_top: false,
            opacity: 1.0,
            icon: None,
            decorated: false,
            closed: GenericSignal::new(),
        }
    }
    /// Adds a child widget to the window.
    ///
    /// Both directions of the link are written: `BaseWidget::add_child` appends to *this* list and
    /// deliberately leaves the child's own parent link alone, and `set_parent` symmetrically does
    /// not walk back. Writing one side only is BLUE22 §B.6 rule 5's defect, and here it is the
    /// window-level one: a child that believes it has no parent is unreachable from the root, so
    /// nothing descending the tree — drawing, hit-testing, focus — finds it. This is the call every
    /// host should make instead of `base.add_child`.
    pub fn add_child(&mut self, child: ObjectId) {
        self.base.add_child_linked(child);
    }
    /// Returns window title.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Updates window title.
    pub fn set_title(&mut self, title: String) {
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }
    /// Returns the title bar height.
    pub fn title_bar_height(&self) -> u32 {
        self.title_bar_height
    }

    /// Sets the title bar height and requests redraw.
    pub fn set_title_bar_height(&mut self, height: u32) {
        if self.title_bar_height == height {
            return;
        }
        self.title_bar_height = height;
        self.base.request_redraw();
    }

    /// Returns the close button size.
    pub fn close_button_size(&self) -> u32 {
        self.close_button_size
    }

    /// Sets the close button size and requests redraw.
    pub fn set_close_button_size(&mut self, size: u32) {
        if self.close_button_size == size {
            return;
        }
        self.close_button_size = size;
        self.base.request_redraw();
    }

    /// Returns the button spacing.
    pub fn button_spacing(&self) -> u32 {
        self.button_spacing
    }

    /// Sets the button spacing and requests redraw.
    pub fn set_button_spacing(&mut self, spacing: u32) {
        if self.button_spacing == spacing {
            return;
        }
        self.button_spacing = spacing;
        self.base.request_redraw();
    }

    /// The material the window asks its client surface to use.
    ///
    /// [`Material::None`] is the default and means "the window paints its own background":
    /// [`Window`] then emits exactly the fill it emitted before materials existed.
    #[cfg(device_profile)]
    pub fn material(&self) -> Material {
        self.material
    }

    /// Requests `material` for the window's client surface.
    ///
    /// This is the semantic request, not a colour: the drawing code asks
    /// [`ThemeManager::material_layer`](crate::theme::ThemeManager::material_layer) for the
    /// layer the active theme assigns to the material and, when the host asked to reduce
    /// transparency, asks for
    /// [`ThemeManager::material_surface`](crate::theme::ThemeManager::material_surface)
    /// instead. A caller therefore never reads the environment or the palette to draw a
    /// material, which is what keeps the accessibility decision in one place.
    ///
    /// The setter requests a redraw only when the value changes, so re-stating the current
    /// material does not schedule a frame.
    #[cfg(device_profile)]
    pub fn set_material(&mut self, material: Material) {
        if self.material == material {
            return;
        }
        self.material = material;
        self.base.request_redraw();
    }

    /// Bytes the platform paints for this window's state, when it can answer.
    ///
    /// `flag` is one of the platform's window-state flags (maximised, minimised, fullscreen,
    /// resizable, decorated). The call is forwarded to the active backend with this widget's id,
    /// so the widget layer and the platform layer finally share one model: the backend both
    /// records the state and, on a desktop, applies it to the native window.
    ///
    /// Returns `false` when the backend cannot honour the flag.
    #[cfg(not(alloc_frugal))]
    pub fn set_window_state(&mut self, flag: crate::platform::WindowStateFlag, on: bool) -> bool {
        crate::platform::runtime::get_platform().set_window_state(self.id(), flag, on)
    }

    /// Returns the backend's answer for `flag`, or `None` when it does not model the state.
    #[cfg(not(alloc_frugal))]
    pub fn window_state(&self, flag: crate::platform::WindowStateFlag) -> Option<bool> {
        crate::platform::runtime::get_platform().is_window_in_state(self.id(), flag)
    }

    /// Requests a minimum client size from the platform and records it as the layout floor.
    ///
    /// [`BaseWidget::min_size`] is the single authoritative store (G-301). This setter forwards
    /// into it and [`Window::min_size`] reads it back, so the platform request and the floor
    /// reported by `Widget::hints` are the same value. The platform call's answer is returned;
    /// the bound is recorded even when the backend refuses it, which preserves the previous
    /// [`Window::min_size`] behaviour of reporting what was requested rather than what the OS
    /// accepted.
    #[cfg(not(alloc_frugal))]
    pub fn set_min_size(&mut self, width: u32, height: u32) -> bool {
        self.base.set_min_size(Some(Size::new(width, height)));
        crate::platform::runtime::get_platform().set_window_min_size(self.id(), width, height)
    }

    /// The minimum client size, read from the authoritative [`BaseWidget::min_size`] store.
    pub fn min_size(&self) -> Option<(u32, u32)> {
        self.base.min_size().map(|size| (size.width, size.height))
    }

    /// Records a maximum client size as the layout ceiling for the host to apply.
    ///
    /// [`BaseWidget::max_size`] is the single authoritative store (G-301): this forwards into it
    /// and [`Window::max_size`] reads it back. It does not request a repaint (G-111): the maximum
    /// is a layout bound, not a pixel-visible fact, and the base setter already treats a
    /// same-value write as a no-op.
    pub fn set_max_size(&mut self, width: u32, height: u32) {
        self.base.set_max_size(Some(Size::new(width, height)));
    }

    /// The maximum client size, read from the authoritative [`BaseWidget::max_size`] store.
    pub fn max_size(&self) -> Option<(u32, u32)> {
        self.base.max_size().map(|size| (size.width, size.height))
    }

    /// Marks the window modal for the host.
    ///
    /// The platform trait has no modality request, so this is a state the host reads when it
    /// decides whether to block input behind the window. It is deliberately not documented as a
    /// window-system call.
    pub fn set_modal(&mut self, modal: bool) {
        if self.modal == modal {
            return;
        }
        self.modal = modal;
        // No repaint (G-111): modality is host intent, not a pixel the draw path reads.
    }

    /// Whether the window is marked modal.
    pub fn is_modal(&self) -> bool {
        self.modal
    }

    /// Marks the window as always-on-top for the host.
    pub fn set_always_on_top(&mut self, always_on_top: bool) {
        if self.always_on_top == always_on_top {
            return;
        }
        self.always_on_top = always_on_top;
        // No repaint (G-111): stacking intent is not a pixel the draw path reads.
    }

    /// Whether the window asks to float above others.
    pub fn is_always_on_top(&self) -> bool {
        self.always_on_top
    }

    /// Sets the window opacity in `0.0..=1.0` (host-facing).
    pub fn set_opacity(&mut self, opacity: f32) {
        if !opacity.is_finite() {
            return;
        }
        let opacity = opacity.clamp(0.0, 1.0);
        if self.opacity == opacity {
            return;
        }
        self.opacity = opacity;
        // No repaint (G-111): opacity is applied by the host, not by this draw path.
    }

    /// The window opacity last set.
    pub fn opacity(&self) -> f32 {
        self.opacity
    }

    /// Requests a window icon from the platform.
    ///
    /// Returns `false` when the backend has no icon path or the path was refused.
    #[cfg(not(alloc_frugal))]
    pub fn set_icon(&mut self, path: &str) -> bool {
        self.icon = Some(path.to_string());
        crate::platform::runtime::get_platform().set_window_icon(self.id(), path)
    }

    /// The icon path last requested.
    pub fn icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// Whether the window paints its own chrome (for a borderless window).
    pub fn is_decorated(&self) -> bool {
        self.decorated
    }

    /// Enables or disables the window's own painted chrome.
    ///
    /// Off by default, because a window the window manager decorates must not paint a second
    /// title bar inside itself. A borderless window turns it on to get a title strip and its
    /// three window buttons.
    pub fn set_decorated(&mut self, decorated: bool) {
        if self.decorated == decorated {
            return;
        }
        self.decorated = decorated;
        self.base.request_redraw();
    }

    /// Emits the window closed signal.
    /// Hides the window and emits the `closed` signal.
    ///
    /// `closed` reports the *lifecycle fact* that the window was closed, not a user action
    /// within the window: the close request may come from a title-bar button, from a
    /// platform event, or from this programmatic call. A host that disabled the window's
    /// contents still needs to run its teardown, so the signal is deliberately not gated by
    /// `enabled` — suppressing it would leak the resources it exists to release.
    pub fn close(&mut self) {
        self.hide();
        self.emit_closed();
    }

    /// The single emission point for the window's close contract (G-303).
    ///
    /// Both signals are emitted together and callers must not repeat the pair: `base.closed` is
    /// what a handle-layer `on_close` (and the JSON/designer `closed` route) connects to, while
    /// [`Window::closed`] is this control's own published event. Emitting only one left the other
    /// listener silent, so every close path — programmatic, painted chrome, and the platform's
    /// `Event::Quit` — routes through here.
    fn emit_closed(&mut self) {
        self.base.closed.emit();
        self.closed.emit();
    }
}
impl Widget for Window {
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
        crate::core::Size::new(640, 480)
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
            "closed" => Some(EventSignalRef::unit("closed", &self.closed)),
            _ => None,
        }
    }
}

/// `Window`'s property contract.
///
/// The three chrome metrics are `u32` fields published as `UInt`; the write path narrows through
/// `expect_u32`, so an out-of-range or negative value is rejected as
/// [`CapabilityAccessError::TypeMismatch`] rather than truncated. `expect_usize` alone was not enough
/// on a 64-bit host: it accepts any `usize`, so a value above `u32::MAX` passed the check and was
/// then silently truncated by the `as u32` that followed.
impl WidgetProperties for Window {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "title_bar_height" => Ok(CapabilityValue::UInt(self.title_bar_height() as u64)),
            "close_button_size" => Ok(CapabilityValue::UInt(self.close_button_size() as u64)),
            "button_spacing" => Ok(CapabilityValue::UInt(self.button_spacing() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "title_bar_height" => {
                self.set_title_bar_height(expect_u32(value)?);
                Ok(())
            }
            "close_button_size" => {
                self.set_close_button_size(expect_u32(value)?);
                Ok(())
            }
            "button_spacing" => {
                self.set_button_spacing(expect_u32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "title_bar_height",
            "close_button_size",
            "button_spacing",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `window` publishes.
    ///
    /// `close` is payload-free and emits the window's `closed` signal, so it
    /// executes here; `set_title` carries the new title and is answered through the
    /// property route, which is why a bare invocation is refused as
    /// [`CapabilityAccessError::OutOfRange`].
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "close" => {
                self.close();
                Ok(())
            }
            "set_title" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Window {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if let Event::MousePress { pos, button, .. } = event {
            if *button == 1 && self.decorated {
                // The close dot is the first of the three painted buttons. Tapping it is the
                // only window action the self-drawn chrome offers: minimise/maximise are the
                // window manager's job and remain reachable through `set_window_state`.
                let dot = self.close_button_size.clamp(6, 20) as i32;
                let y = (self.title_bar_height as i32 - dot) / 2;
                let close = Rect::new(8, y, dot as u32, dot as u32);
                if close.contains_point(*pos) {
                    self.close();
                    return;
                }
            }
        }
        if matches!(event, Event::Quit) {
            self.emit_closed();
        }
    }
}

impl Draw for Window {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        // The client fill reads the **active theme's** background, not a literal.
        //
        // The literal here was `rgb(240, 240, 240)`, which is exactly the light
        // preset's `colors.background` — so the window happened to look right in
        // light and rendered unchanged in dark. Two things were wrong at once and
        // both were invisible: the window did not follow a theme switch (rule
        // #104), and its fill was byte-identical to the census frame background,
        // so the rendering census could not tell it apart from an empty surface.
        //
        // Precedence is unchanged: an explicit style wins, then the theme, then the
        // literal as a last resort for a manager with no active theme. A stripped device
        // build has no theme module, so the literal is its only rung.
        #[cfg(device_profile)]
        let themed_background = crate::style::palette_shared().map(|colors| colors.background);
        #[cfg(not(device_profile))]
        let themed_background: Option<Color> = None;
        let bg_color =
            style.background_color.or(themed_background).unwrap_or(Color::rgb(240, 240, 240));

        // Only the client area is painted: the background.
        //
        // # Why the title bar, window buttons and border were removed
        //
        // This used to paint a title bar in the window's top 32px, a red close button
        // at the right edge, and minimize/maximize buttons beside it. Those are the
        // **window manager's** chrome: every desktop the library targets already draws
        // a decorated frame, so the painted copies appeared *inside* a real title bar —
        // a second, fake one, with a red X that looks like a close button and does
        // nothing. The library never received clicks for it, because the OS owned those
        // pixels, so the only thing the drawing achieved was looking broken.
        //
        // The border went the same way, for a second reason beyond being chrome: it was
        // stroked from this widget's **absolute** rectangle, and a stroked rectangle
        // covers every pixel it crosses. A window created at `(100, 100)` therefore had
        // its border drawn across the whole client area, over the controls — a corner of
        // the window read as the border colour rather than the background. The window
        // manager draws the real border; the client area is the controls'.
        //
        // The geometry the rest of the window keys off is unaffected: a layout that
        // wants to start below the title bar uses `Window::title_bar_height`, which is
        // still reported. What changed is only that the window no longer *paints* chrome.
        //
        // # Why the rectangle is normalised to the client origin
        //
        // `geometry()` is in **screen** coordinates (the position the OS was asked for),
        // while the controls inside are in **client** coordinates. Painting the fill at
        // the screen position would leave the area the controls actually occupy
        // uncovered — which, once the children were drawn correctly, is exactly the
        // top-left strip that would then show through as the clear colour. Normalising to
        // the client origin makes the fill match the space the children use.
        let client = Rect::new(0, 0, rect.width, rect.height);

        // A material replaces the window's own background fill with a composed surface.
        //
        // # Why the fill is *not* painted inside the layer
        //
        // A layer captures what is behind the window, blurs it, applies the material's tint and
        // then composites its foreground over the result. Painting the opaque theme background
        // inside the layer would cover the blurred backdrop, turning the material back into an
        // ordinary fill and making `reduce_transparency` unobservable. The window therefore
        // contributes no opaque foreground of its own: the layer *is* its surface, and controls
        // hosted by the window are painted by the runtime as their own draw calls.
        //
        // # The reduced-transparency branch
        //
        // When the host asked for opaque surfaces, `material_layer` answers `None` and the arm
        // below draws the palette colour that stands in for the material. Branching on the
        // theme's resolution rather than reading the environment here is what keeps the
        // accessibility decision in one place.
        //
        // # The default is byte-identical
        //
        // `Material::None` skips this block entirely and falls through to the original single
        // fill, so a window that never asks for a material emits exactly the commands (and
        // therefore the pixels) it emitted before.
        #[cfg(device_profile)]
        {
            let material = self.material;
            if material != Material::None {
                let manager = crate::style::theme_manager();
                match manager.material_layer(material) {
                    Some((blur_radius, tint)) => {
                        context.push_material_layer(client, blur_radius, tint);
                        context.pop_layer();
                    }
                    None => context.fill_rect(client, manager.material_surface(material)),
                }
                return;
            }
        }
        context.fill_rect(client, bg_color);

        // Self-drawn chrome for a borderless window. It is opt-in precisely because a
        // window-manager-decorated window must not paint a second title bar inside itself.
        if self.decorated {
            let bar_height = self.title_bar_height.min(rect.height);
            let bar = Rect::new(0, 0, rect.width, bar_height);
            let title_fill = bg_color.blend(&bg_color.contrast_color(), 0.06);
            context.fill_rect(bar, title_fill);
            let dot = self.close_button_size.clamp(6, 20) as i32;
            let y = (bar_height as i32 - dot) / 2;
            let gap = (self.button_spacing.max(self.close_button_size + 6)) as i32;
            for (index, color) in [
                Color::rgba(255, 95, 86, 255),
                Color::rgba(255, 189, 46, 255),
                Color::rgba(39, 201, 63, 255),
            ]
            .into_iter()
            .enumerate()
            {
                let x = 8 + index as i32 * gap;
                context.fill_circle_aa(Point::new(x + dot / 2, y + dot / 2), dot as u32 / 2, color);
            }
            if !self.title.is_empty() {
                let font = crate::widget::metrics::role_font(self.style(), 13.0);
                let line = context.text_line(bar, &font);
                // Borrow the title: the fitted helper returns a `Cow`, so cloning the `String`
                // here was an allocation per frame for a value that is only read (G-320).
                context.draw_text_fitted_cow(
                    line,
                    &self.title,
                    &font,
                    bg_color.contrast_color(),
                    crate::core::HorizontalAlignment::Center,
                );
            }
        }
    }
}
// NOTE: The show() method is now handled by platform backend.
// For full application integration, use the platform event loop via crate::run().
// The platform backend (macOS: NSApp().run(), Windows: message loop, etc.)
// handles all event dispatch and rendering coordination.

#[cfg(all(test, device_profile))]
mod material_draw_tests {
    use super::*;
    use crate::compat::Vec;
    use crate::core::Font;
    use crate::render::{
        LayerEffect, PaintBackend, RenderCommand, RenderContext, ShapedText, SoftwarePaintBackend,
        SoftwareRenderConfig, TextMetrics,
    };
    use crate::style::environment::{
        install_environment, uninstall_environment, DefaultEnvironment, EnvironmentProvider,
    };
    use crate::theme::Material;
    use alloc::boxed::Box;

    /// A backend that records every command in draw order and still rasterises it.
    ///
    /// Wrapping the software backend is what lets one draw be asserted **both** as a command
    /// stream (did the window request a material layer?) and as pixels (did the default material
    /// change what is painted?). A recorder that only counted commands would miss a change in the
    /// fill, and a pixel-only assertion could not see a layer that composites to the same colour.
    struct Recorder {
        commands: Vec<RenderCommand>,
        inner: SoftwarePaintBackend,
    }

    impl Recorder {
        fn new(size: Size) -> Self {
            Self { commands: Vec::new(), inner: SoftwarePaintBackend::new(size, 1.0) }
        }
    }

    impl PaintBackend for Recorder {
        fn begin_frame(&mut self, clear: Color) {
            self.inner.begin_frame(clear);
        }
        fn end_frame(&mut self) {
            self.inner.end_frame();
        }
        fn execute_command(&mut self, command: &RenderCommand) {
            self.commands.push(command.clone());
            self.inner.execute_command(command);
        }
        fn size(&self) -> Size {
            self.inner.size()
        }
        fn set_size(&mut self, size: Size) {
            self.inner.set_size(size);
        }
        fn dpi_scale(&self) -> f32 {
            self.inner.dpi_scale()
        }
        fn set_dpi_scale(&mut self, scale: f32) {
            self.inner.set_dpi_scale(scale);
        }
        fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
            self.inner.measure_text(text, font)
        }
        fn shape_text(&self, text: &str, font: &Font) -> crate::compat::Rc<ShapedText> {
            self.inner.shape_text(text, font)
        }
        fn frame_rgba(&self) -> &[u8] {
            self.inner.frame_rgba()
        }
        fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
            self.inner.apply_render_config(config);
        }
    }

    /// Draws `window` once and returns its command stream and final pixels.
    fn render(window: &mut Window) -> (Vec<RenderCommand>, Vec<u8>) {
        let mut backend = Recorder::new(Size::new(320, 240));
        backend.begin_frame(Color::BLACK);
        {
            let mut context = RenderContext::new(&mut backend);
            window.draw(&mut context);
        }
        backend.end_frame();
        let pixels = backend.inner.frame_rgba().to_vec();
        (backend.commands, pixels)
    }

    fn is_layer_push(command: &RenderCommand) -> bool {
        matches!(command, RenderCommand::PushLayer { .. })
    }

    /// The default material is the identity: it emits the same pixels and the same single fill
    /// as an explicit `Material::None`, and never opens a layer.
    #[test]
    fn the_default_material_paints_the_same_bytes_as_none() {
        let _guard = crate::theme::theme_test_guard();
        let frame = Rect::new(0, 0, 320, 240);

        let mut implicit = Window::new("implicit".to_string(), frame);
        assert_eq!(implicit.material(), Material::None, "a new window asks for no material");
        let (implicit_commands, implicit_pixels) = render(&mut implicit);

        let mut explicit = Window::new("explicit".to_string(), frame);
        explicit.set_material(Material::None);
        let (explicit_commands, explicit_pixels) = render(&mut explicit);

        assert_eq!(
            implicit_pixels, explicit_pixels,
            "the default material must paint byte-identical pixels to an explicit None"
        );
        assert!(
            implicit_commands.iter().all(|command| !is_layer_push(command)),
            "Material::None must not request a material layer"
        );
        assert_eq!(
            implicit_commands.len(),
            explicit_commands.len(),
            "the default and an explicit None must emit the same command stream shape"
        );
        assert_eq!(
            implicit_commands
                .iter()
                .filter(|command| matches!(command, RenderCommand::FillRect { .. }))
                .count(),
            1,
            "the default window is its own background and nothing else"
        );
    }

    /// A non-default material requests the theme's backdrop layer for the client rect instead of
    /// painting the opaque theme background.
    #[test]
    fn a_material_window_requests_a_backdrop_layer() {
        let _guard = crate::theme::theme_test_guard();
        // `DefaultEnvironment` reports no reduced transparency, so the material must ask for a
        // layer rather than degrade.
        let previous = install_environment(Box::new(DefaultEnvironment));

        let mut window = Window::new("material".to_string(), Rect::new(0, 0, 320, 240));
        window.set_material(Material::Sidebar);
        assert_eq!(window.material(), Material::Sidebar);
        let (commands, _) = render(&mut window);

        let (bounds, effect) = commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::PushLayer { bounds, effect } => Some((*bounds, *effect)),
                _ => None,
            })
            .expect("a material window must request a backdrop layer");
        assert_eq!(bounds, Rect::new(0, 0, 320, 240), "the layer covers the client area");
        match effect {
            Some(LayerEffect::BackdropBlurTint { radius, .. }) => assert_eq!(
                radius,
                Material::Sidebar
                    .backdrop_blur_radius()
                    .expect("a sidebar material has a blur radius"),
                "the requested blur is the material's own radius"
            ),
            other => {
                panic!("a sidebar material must ask for a tinted backdrop blur, got {other:?}")
            }
        }
        assert!(
            commands.iter().any(|command| matches!(command, RenderCommand::PopLayer)),
            "every pushed layer must be popped in the same draw"
        );
        assert!(
            !commands.iter().any(|command| matches!(command, RenderCommand::FillRect { .. })),
            "the window contributes no opaque fill under a composed material"
        );

        drop(previous);
        uninstall_environment();
    }

    /// Reduced transparency removes the layer and paints the palette surface the material
    /// degrades to.
    #[test]
    fn reduced_transparency_paints_the_opaque_material_surface() {
        struct ReducedTransparency;
        impl EnvironmentProvider for ReducedTransparency {
            fn reduce_transparency(&self) -> bool {
                true
            }
        }

        let _guard = crate::theme::theme_test_guard();
        let previous = install_environment(Box::new(ReducedTransparency));

        // Read the expected colour while holding the manager, then release it before drawing: the
        // draw path takes the same lock, and a guard held across it would deadlock.
        let expected = { crate::style::theme_manager().material_surface(Material::Sidebar) };
        assert_eq!(expected.a, 255, "the degraded surface is opaque");

        let mut window = Window::new("reduced".to_string(), Rect::new(0, 0, 320, 240));
        window.set_material(Material::Sidebar);
        let (commands, _) = render(&mut window);

        assert!(
            !commands.iter().any(is_layer_push),
            "a host that reduced transparency must not receive a backdrop layer"
        );
        let fill = commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::FillRect { rect, color } => Some((*rect, *color)),
                _ => None,
            })
            .expect("a degraded material is painted as an opaque fill");
        assert_eq!(fill.0, Rect::new(0, 0, 320, 240));
        assert_eq!(fill.1, expected, "the fill is the material's palette surface");

        drop(previous);
        uninstall_environment();
    }
}

#[cfg(test)]
mod window_contract_tests {
    use super::*;
    use crate::widget::svg::render_to_svg;

    /// G-301: one authoritative store. The host-facing pair and the base/layout path read and
    /// write the same bound, in both directions.
    ///
    /// The host-facing pair is `Window::set_min_size(width, height)` / `icon()`'s setter, which a
    /// stripped build does not carry (`#[cfg(not(alloc_frugal))]`): with no platform singleton
    /// there is nothing to ask, so the round trip it asserts has no `mini` spelling.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn window_size_bounds_share_the_base_store() {
        let mut window = Window::new("Doc".to_string(), Rect::new(0, 0, 400, 300));

        let _ = window.set_min_size(320, 240);
        assert_eq!(window.min_size(), Some((320, 240)));
        assert_eq!(window.base().min_size(), Some(Size::new(320, 240)));

        window.base_mut().set_min_size(Some(Size::new(300, 200)));
        assert_eq!(window.min_size(), Some((300, 200)), "the window getter reads the base store");

        window.set_max_size(800, 600);
        assert_eq!(window.max_size(), Some((800, 600)));
        assert_eq!(window.base().max_size(), Some(Size::new(800, 600)));
        assert_eq!(window.hints().width.max, 800, "the layout ceiling is the same store");
    }

    /// G-303: both lifecycle signals are emitted from one point, so every close path fires both.
    #[test]
    fn every_close_path_emits_both_closed_signals() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut window = Window::new("Doc".to_string(), Rect::new(0, 0, 400, 300));
        let base_hits = Arc::new(AtomicUsize::new(0));
        let own_hits = Arc::new(AtomicUsize::new(0));
        let base_counter = base_hits.clone();
        window.base.closed.connect(move || {
            base_counter.fetch_add(1, Ordering::SeqCst);
        });
        let own_counter = own_hits.clone();
        window.closed.connect(move || {
            own_counter.fetch_add(1, Ordering::SeqCst);
        });

        window.close();
        assert_eq!(base_hits.load(Ordering::SeqCst), 1);
        assert_eq!(own_hits.load(Ordering::SeqCst), 1);

        // The platform-driven path goes through the same emission point.
        window.handle_event(&Event::Quit);
        assert_eq!(base_hits.load(Ordering::SeqCst), 2);
        assert_eq!(own_hits.load(Ordering::SeqCst), 2);
    }

    #[cfg(not(alloc_frugal))]
    #[test]
    fn host_facing_window_facts_round_trip() {
        let mut window = Window::new("Doc".to_string(), Rect::new(0, 0, 400, 300));
        assert!(!window.is_modal());
        window.set_modal(true);
        assert!(window.is_modal());
        assert!(!window.is_always_on_top());
        window.set_always_on_top(true);
        assert!(window.is_always_on_top());
        window.set_opacity(0.5);
        assert_eq!(window.opacity(), 0.5);
        window.set_max_size(800, 600);
        assert_eq!(window.max_size(), Some((800, 600)));
        assert_eq!(window.min_size(), None);
        let _ = window.set_min_size(320, 240);
        assert_eq!(window.min_size(), Some((320, 240)));
        assert_eq!(window.icon(), None);
        window.set_icon("/tmp/doc.png");
        assert_eq!(window.icon(), Some("/tmp/doc.png"));
    }

    #[test]
    fn self_drawn_chrome_is_opt_in_and_clickable() {
        let mut window = Window::new("Doc".to_string(), Rect::new(0, 0, 400, 300));
        assert!(!window.is_decorated());
        window.set_decorated(true);
        assert!(window.is_decorated());
        let closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let c = closed.clone();
        window.closed.connect(move || {
            c.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        window.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(12, 16),
            button: 1,
            modifiers: 0,
        });
        assert!(closed.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn a_plain_window_still_paints_only_its_client_fill() {
        let mut window = Window::new("Doc".to_string(), Rect::new(0, 0, 400, 300));
        let plain = render_to_svg(&mut window);
        window.set_decorated(true);
        let chrome = render_to_svg(&mut window);
        assert!(plain.starts_with("<svg"));
        assert_ne!(plain, chrome, "the chrome is opt-in and visibly different");
    }
}
