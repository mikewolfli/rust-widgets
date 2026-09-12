//! Custom drawing trait for widgets that want to render their own content.

use crate::render::RenderContext;

/// Custom drawing trait for widgets that want to render their own content.
/// Widgets implementing this trait can provide custom drawing logic instead of
/// relying solely on native platform rendering.
pub trait Draw {
    /// Draw the widget's content using the provided render context.
    /// This method is called when the widget needs to be repainted.
    fn draw(&mut self, context: &mut RenderContext);
    /// Returns true if this widget uses custom drawing, false for native rendering.
    /// This allows the rendering system to choose between native and custom paths.
    /// Defaults to `false` because most platform widgets should use native rendering.
    fn uses_custom_drawing(&self) -> bool {
        false
    }
}

/// Implements [`crate::widget::Widget::as_draw_mut`] for a widget that paints itself.
///
/// # Why a macro and not a blanket impl
///
/// `impl<T: Draw + Widget> Widget for T` would collide with every concrete
/// `impl Widget for Foo` in the crate (coherence forbids both), and a blanket
/// `impl<T: Draw> Draw for T` cannot reach the `Self: Widget + 'static` bound
/// needed to cast to `&mut dyn Any`. The remaining option is an explicit,
/// per-type opt-in — which is also the honest design: a widget saying "yes, I
/// paint myself" should be a deliberate, visible statement.
///
/// Every widget that has an `impl Draw for X` belongs in its `impl Widget for X`:
///
/// ```
/// use rust_widgets::core::Rect;
/// use rust_widgets::widget::special_widgets::code_editor::CodeEditor;
/// use rust_widgets::widget::{Draw, Widget};
///
/// let mut editor = CodeEditor::new(Rect::new(0, 0, 80, 40));
/// // `as_draw_mut` reaches the widget's `Draw` implementation.
/// assert!(editor.as_draw_mut().is_some());
/// ```
#[macro_export]
macro_rules! impl_self_drawn {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl $crate::widget::Widget for $ty {
                fn as_draw_mut(&mut self) -> ::core::option::Option<&mut dyn $crate::widget::Draw> {
                    ::core::option::Option::Some(self)
                }
            }
        )+
    };
}
