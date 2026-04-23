//! Core `App` type — lifecycle wrapper.

use crate::core::ObjectId;
use crate::WidgetTriggerEvent;

use super::handle::WindowHandle;

/// High-level application wrapper that manages the event loop lifecycle.
///
/// # Example
///
/// ```ignore
/// use rust_widgets::app::App;
///
/// let app = App::new();
/// app.init();
///
/// let win = app.new_window("Hello", 100, 100, 640, 480);
/// let btn = win.new_button("Click me", 10, 10, 120, 32);
///
/// app.run();
/// ```
pub struct App {
    _private: (),
}

impl App {
    /// Create a new application handle.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Initialize global platform and i18n subsystems.
    pub fn init(&self) {
        crate::init();
    }

    /// Run the platform main event loop (blocks).
    pub fn run(&self) {
        crate::run();
    }

    /// Request the event loop to shut down.
    pub fn quit(&self) {
        crate::quit();
    }

    /// Create a top-level window and return a type-safe handle.
    pub fn new_window(
        &self,
        title: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> WindowHandle {
        WindowHandle::from_raw(crate::create_window(title, x, y, w, h))
    }

    /// Poll the next triggered event from the platform layer.
    pub fn poll_event(&self) -> Option<WidgetTriggerEvent> {
        crate::poll_widget_trigger_event()
    }

    /// Poll the raw ObjectId of the most recently triggered widget.
    pub fn poll_triggered(&self) -> Option<ObjectId> {
        crate::poll_widget_triggered()
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
