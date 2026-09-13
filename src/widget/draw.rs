// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Custom drawing trait for widgets that want to render their own content.

use crate::render::RenderContext;

/// Custom drawing trait for widgets that want to render their own content.
/// Widgets implementing this trait can provide custom drawing logic instead of
/// relying solely on platform rendering.
pub trait Draw {
    /// Draw the widget's content using the provided render context.
    /// This method is called when the widget needs to be repainted.
    fn draw(&mut self, context: &mut RenderContext);
    /// Returns true if this widget uses custom drawing, false otherwise.
    /// This allows the rendering system to choose between the two paths.
    /// Defaults to `false` because most widgets map onto a platform control.
    fn uses_custom_drawing(&self) -> bool {
        false
    }
}
