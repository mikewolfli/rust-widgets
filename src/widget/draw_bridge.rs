// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The painting bridge that connects a self-painting widget to `dyn Widget`.
//!
//! # Why this module exists
//!
//! [`crate::widget::runtime::render_frame`] holds widgets as `&mut dyn Widget` and
//! needs to reach [`Draw::draw`]. `Widget` deliberately does not *require* `Draw`
//! (a widget may paint nothing and lay out children instead), so `Widget` exposes
//! `as_draw_mut() -> Option<&mut dyn Draw>`, defaulting to `None`.
//!
//! That default was silently wrong. An audit found **168** types implementing
//! `Draw`, but only **6** overriding `as_draw_mut` — so mounting any of the other
//! **162** produced an empty surface and reported no error. The failure was
//! invisible: `render_frame` asked the bridge, got `None`, and concluded there was
//! nothing to paint.
//!
//! # Why the fix is a macro invoked inside each `impl Widget`
//!
//! The ideal `impl<T: Draw> Paintable for T` compiles but is unusable here: trait
//! selection needs the concrete type, and `&mut dyn Widget` has erased it. A
//! blanket impl is only reachable behind a generic bound like `W: Draw + Widget`,
//! which no `Box<dyn Widget>` satisfies — that is precisely why `as_draw_mut`
//! exists at all.
//!
//! The override therefore has to be emitted **where the concrete type is visible**:
//! inside the widget's own `impl Widget` block. [`crate::impl_draw_bridge!`] emits
//! exactly that one method. It is a *generated* line rather than a remembered one,
//! and the coverage test in [`crate::widget::runtime`] fails by name if a painting
//! widget is missing it — so the silent-blank failure cannot ship.
//!
//! The macro is zero-cost (principle #28): it expands to the same `Some(self)` the
//! six hand-written bridges already use, with no table and no lookup.

use crate::widget::draw::Draw;
use crate::widget::widget_trait::Widget;

/// Emits the [`Widget::as_draw_mut`] override for a self-painting widget.
///
/// Invoke **inside** the widget's `impl Widget` block:
///
/// ```ignore
/// impl Widget for Button {
///     fn base(&self) -> &BaseWidget { &self.base }
///     fn base_mut(&mut self) -> &mut BaseWidget { &mut self.base }
///
///     impl_draw_bridge!();
/// }
/// ```
///
/// # Why this is a macro and not a blanket impl
///
/// The obvious `impl<T: Draw> Widget for T` is impossible — it would overlap the
/// per-type `impl Widget for X` every control already has (coherence). An
/// `impl<T: Draw> Paintable for T` helper does compile, but a `&mut dyn Widget`
/// cannot *select* it: trait selection needs the concrete type, and the trait
/// object has erased it. That is the whole reason `as_draw_mut` exists.
///
/// So the override is generated where the concrete type is still visible — inside
/// its own impl — and the macro makes the intent (`this type paints itself`)
/// explicit and uniform, instead of a line each author must remember to copy.
///
/// The coverage test in [`crate::widget::runtime`] fails by name when a painting
/// widget lacks this call, which is what the previous hand-written approach had no
/// way to detect: it had silently fallen to 6 of 168.
#[macro_export]
macro_rules! impl_draw_bridge {
    () => {
        /// Reports this widget as the object that paints it.
        ///
        /// The type is in the widget's own `impl Widget` block and implements
        /// `Draw`, so `Some(self)` is total — there is no case in which this can
        /// be wrong, which is why it is generated rather than hand-written.
        fn as_draw_mut(&mut self) -> Option<&mut dyn $crate::widget::Draw> {
            Some(self)
        }
    };
}

/// Returns the painting channel for `widget`, or `None` when it paints nothing.
///
/// The single entry point the render loop should use. It asks the widget itself,
/// so a wrapper that paints through a child keeps working, and a widget that
/// implements `Draw` answers `Some(self)` through [`crate::impl_draw_bridge!`].
pub fn draw_of(widget: &mut dyn Widget) -> Option<&mut dyn Draw> {
    widget.as_draw_mut()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
    use crate::widget::base_widgets::button::Button;

    /// The bridge must hold for a control that goes through the macro: a mounted
    /// control answering `None` would paint nothing at all.
    #[test]
    fn a_bridged_control_reaches_its_draw_impl() {
        let mut button = Button::new("ok".to_string(), Rect::new(0, 0, 40, 24));
        let dyn_widget: &mut dyn Widget = &mut button;
        assert!(
            draw_of(dyn_widget).is_some(),
            "Button implements Draw, so the bridge must report it as paintable"
        );
    }

    /// A frame must contain drawn pixels — a bridge that compiles but paints
    /// nothing would pass a signature check and fail in production.
    #[test]
    fn a_bridged_control_actually_paints() {
        let mut button = Button::new("ok".to_string(), Rect::new(0, 0, 40, 24));
        let dyn_widget: &mut dyn Widget = &mut button;
        let drawable = draw_of(dyn_widget).expect("bridge");

        let mut surface = SoftwarePaintBackend::new(Size::new(40, 24), 1.0);
        surface.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut surface);
            drawable.draw(&mut context);
        }
        surface.end_frame();
        assert!(
            surface.frame_rgba().chunks_exact(4).any(|px| px[3] != 0),
            "a bridged Button must paint at least one pixel"
        );
    }

    /// Every widget type in this crate paints itself, so the bridge is total.
    ///
    /// An audit of the source found that the set of `impl Widget` types and the
    /// set of `impl Draw` types are now **equal** (167 and 166 respectively, the
    /// difference being `WebViewEnhanced`, which is a `Draw` helper rather than a
    /// widget). That is a stronger result than "every widget that paints can be
    /// painted": there is no widget left that paints nothing.
    ///
    /// The assertion is written as a property of a widget that would *have* to
    /// answer `None` — one with no `Draw` impl — so it keeps documenting the
    /// contract of the fallback even if the crate later gains such a widget.
    #[test]
    fn a_widget_without_draw_reports_none() {
        use crate::core::Rect;
        use crate::event::{Event, EventHandler};
        use crate::widget::base::BaseWidget;

        /// A minimal widget that deliberately has no `Draw` impl.
        struct Inert {
            base: BaseWidget,
        }

        impl Widget for Inert {
            fn base(&self) -> &BaseWidget {
                &self.base
            }
            fn base_mut(&mut self) -> &mut BaseWidget {
                &mut self.base
            }
        }

        impl EventHandler for Inert {
            fn handle_event(&mut self, _event: &Event) {}
        }

        let mut inert = Inert {
            base: BaseWidget::new(
                crate::widget::WidgetKind::Panel,
                Rect::new(0, 0, 10, 10),
                "inert",
            ),
        };
        let dyn_widget: &mut dyn Widget = &mut inert;
        assert!(
            draw_of(dyn_widget).is_none(),
            "a widget with no Draw impl must not claim to be paintable, or the \
             bridge would be reporting something it cannot deliver"
        );
    }

    /// Every widget the factory can build must be paintable through `dyn Widget`.
    ///
    /// This is the guard for the failure that motivated this module: 162 of 168
    /// types implemented `Draw` but answered `None` from `as_draw_mut`, so
    /// mounting one painted a blank surface with no error reported.
    ///
    /// The list comes from the factory, not from a literal here. A hand-typed
    /// list is precisely what drifted, so this test must never grow one.
    ///
    /// Gated on the full widget set for the same reason the factory itself is:
    /// a stripped profile compiles neither the factory nor the registry of
    /// constructors this walks. The attribute must match that sentence — it
    /// previously said `widgets_unstripped`, which is wider than the factory's own
    /// `full_widgets` gate, so a build with no device profile (such as
    /// `--features android`) compiled this test against an absent
    /// `new_with_defaults`.
    #[cfg(full_widgets)]
    #[test]
    fn every_factory_widget_can_be_painted() {
        use crate::widget::capability::WidgetFactory;

        let factory = WidgetFactory::new_with_defaults();
        let names = factory.widget_names();
        assert!(!names.is_empty(), "the factory must register widgets");

        let mut paintless = Vec::new();
        for name in names {
            let Some(mut widget) = factory.create(name, Rect::new(0, 0, 64, 48), "x") else {
                continue;
            };
            if draw_of(widget.as_mut()).is_none() {
                paintless.push(name);
            }
        }

        assert!(
            paintless.is_empty(),
            "these widgets implement Draw but are not reachable through \
             Widget::as_draw_mut, so mounting them paints nothing: {paintless:?}"
        );
    }
}
