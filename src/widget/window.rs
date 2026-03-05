//! Window widget and platform integration.

use crate::core::{ObjectId, Rect};
use crate::widget::{BaseWidget, WidgetKind};

/// Main application window.
pub struct Window {
    base: BaseWidget,
    title: String,
}

impl Window {
    /// Creates a new window with title and geometry.
    pub fn new(title: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Window, geometry, "Window"),
            title,
        }
    }

    /// Adds a child widget to the window.
    pub fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }

    /// Shows the window using winit backend.
    pub fn show(&self) {
        use winit::event::{Event, WindowEvent};
        use winit::event_loop::{ControlFlow, EventLoop};
        use winit::window::WindowBuilder;

        let event_loop = EventLoop::new();
        let geometry = self.geometry();
        let window = WindowBuilder::new()
            .with_title(self.title())
            .with_inner_size(winit::dpi::LogicalSize::new(
                geometry.width as f64,
                geometry.height as f64,
            ))
            .with_position(winit::dpi::LogicalPosition::new(
                geometry.x as f64,
                geometry.y as f64,
            ))
            .build(&event_loop)
            .expect("Failed to create window");

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    _ => {}
                },
                _ => {}
            }
        });
    }

    /// Returns window geometry.
    pub fn geometry(&self) -> Rect {
        self.base.geometry()
    }

    /// Returns window title.
    pub fn title(&self) -> &str {
        &self.title
    }
}

// NOTE: The show() method uses winit for standalone window display.
// For full application integration, use the platform event loop via crate::run().
// The platform backend (macOS: NSApp().run(), Windows: message loop, etc.)
// handles all event dispatch and rendering coordination.
