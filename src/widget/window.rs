// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Window widget and platform integration.
use crate::compat::{String, ToString};
use crate::core::{Color, ObjectId, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::capability::coercion::{expect_string, expect_usize};
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
            closed: GenericSignal::new(),
        }
    }
    /// Adds a child widget to the window.
    pub fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    /// Returns window title.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Updates window title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.base.request_redraw();
    }
    /// Returns the title bar height.
    pub fn title_bar_height(&self) -> u32 {
        self.title_bar_height
    }

    /// Sets the title bar height and requests redraw.
    pub fn set_title_bar_height(&mut self, height: u32) {
        self.title_bar_height = height;
        self.base.request_redraw();
    }

    /// Returns the close button size.
    pub fn close_button_size(&self) -> u32 {
        self.close_button_size
    }

    /// Sets the close button size and requests redraw.
    pub fn set_close_button_size(&mut self, size: u32) {
        self.close_button_size = size;
        self.base.request_redraw();
    }

    /// Returns the button spacing.
    pub fn button_spacing(&self) -> u32 {
        self.button_spacing
    }

    /// Sets the button spacing and requests redraw.
    pub fn set_button_spacing(&mut self, spacing: u32) {
        self.button_spacing = spacing;
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
        self.closed.emit();
    }
}
impl Widget for Window {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(640, 480)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Window`'s property contract.
///
/// The three chrome metrics are `u32` fields published as `UInt`; the write path
/// narrows through `expect_usize` first, so an out-of-range or negative value is
/// rejected as [`CapabilityAccessError::TypeMismatch`] rather than truncated.
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
                self.set_title_bar_height(expect_usize(value)? as u32);
                Ok(())
            }
            "close_button_size" => {
                self.set_close_button_size(expect_usize(value)? as u32);
                Ok(())
            }
            "button_spacing" => {
                self.set_button_spacing(expect_usize(value)? as u32);
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
        if matches!(event, Event::Quit) {
            self.closed.emit();
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
        let themed_background =
            crate::style::theme_manager().current_theme().map(|active| active.colors.background);
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
        context.fill_rect(client, bg_color);
    }
}
// NOTE: The show() method is now handled by platform backend.
// For full application integration, use the platform event loop via crate::run().
// The platform backend (macOS: NSApp().run(), Windows: message loop, etc.)
// handles all event dispatch and rendering coordination.
