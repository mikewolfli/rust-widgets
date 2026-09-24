// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The painting bridge that connects a self-painting widget to `dyn Widget`.
//!
//! # Why this module exists
//!
//! `crate::widget::runtime::render_frame` holds widgets as `&mut dyn Widget` and
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
//! and the coverage test in `crate::widget::runtime` fails by name if a painting
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
/// The coverage test in `crate::widget::runtime` fails by name when a painting
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

/// Implements `Default` by delegating to a type's `new()`.
///
/// # Why this exists
///
/// A type whose `new()` takes its geometry has no universal default — but many
/// types in this crate have a zero-argument `new()`, and for those the impl is
/// always the identical five lines:
///
/// ```ignore
/// impl Default for Thing {
///     fn default() -> Self {
///         Self::new()
///     }
/// }
/// ```
///
/// That block appeared **117 times**, which is 117 copies of one fact. It is also
/// the kind of boilerplate that silently drifts: a `new()` that gains a required
/// argument leaves a `Default` impl that no longer describes it, because nothing
/// forces the two to be read together.
///
/// # Usage
///
/// ```ignore
/// impl Thing {
///     pub fn new() -> Self { /* … */ }
/// }
///
/// crate::impl_default_via_new!(Thing);
/// ```
///
/// The macro generates an inherent-free impl, so it can be invoked anywhere in the
/// module that defines the type — normally immediately after the type's own `impl`
/// block, so the two are read together.
#[macro_export]
macro_rules! impl_default_via_new {
    ($type:ty) => {
        impl ::core::default::Default for $type {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

/// Returns the painting channel for `widget`, or `None` when it paints nothing.
///
/// The single entry point the render loop should use. It asks the widget itself,
/// so a wrapper that paints through a child keeps working, and a widget that
/// implements `Draw` answers `Some(self)` through [`crate::impl_draw_bridge!`].
///
/// # Painting is also where a **host-owned** control is advanced
///
/// [`crate::widget::runtime::tick_animations`] sweeps the mounted registry. A control held as a
/// `Box<dyn Widget>` is not in it, so nothing advanced it and nothing knew it was in flight: a
/// `Switch` set to ON and then painted showed its thumb at the off end every frame, forever, and
/// reported no error. That is not a hypothetical host — it is how `census`,
/// `examples/export_control_svgs.rs` and this function's own callers hold their controls, and it is
/// the pattern the crate's front-page example documents.
///
/// So the paint path advances such a control and records the fact. Both halves are needed:
/// advancing is what makes the picture move, and recording is what lets
/// [`crate::widget::runtime::animation_bus_needs_another_frame`] answer `true`, so a loop learns
/// the frame it would otherwise never schedule is needed.
///
/// # The two exclusions, and why each is load-bearing
///
/// **A mounted control is not advanced here.** It reaches this function every frame too, and
/// advancing it would run its animation once for `tick_animations` and once for its own paint —
/// twice per frame, the exact failure `tick_animations`' documentation warns about ("the same
/// button advanced twice in one frame"). [`crate::widget::runtime::is_mounted`] keeps the two
/// populations disjoint.
///
/// **A control that manages its own repaint is not advanced here either.** Such a control's
/// `tick` calls [`crate::widget::BaseWidget::request_redraw`], so advancing it on every paint
/// would make every advance ask for the next paint: a feedback loop with no way to stop, and
/// nothing outside this function can see that the owner is already choosing when to repaint.
/// [`crate::widget::Widget::manages_own_repaint`] reports that fact and defaults to "no", so an
/// ordinary control — a hovered `Button`, a toggled `Switch`, the case that made this necessary —
/// is advanced, while a free-running `Spinner` draws whatever frame it is on and keeps its own
/// cadence. The observable consequence for the snapshots is that two consecutive renders of the
/// same control are identical, which the exporter's reproducibility depends on.
///
/// A resting owned control costs one `is_animating()` call, which is what keeps the "a still
/// window pays nothing" property of the bus intact.
pub fn draw_of(widget: &mut dyn Widget) -> Option<&mut dyn Draw> {
    advance_host_owned_control(widget);
    widget.as_draw_mut()
}

/// Advances an owned control that owes frames, and tells the bus so.
///
/// Split from [`draw_of`] so the **whole** of the animation-bus interaction — the frame step, the
/// `is_mounted` question and the two bus calls — is one call site that a build without the runtime
/// can compile out. `mini` and `embedded` are `alloc_frugal`: they have no `widget::runtime` (the
/// module is `#[cfg(not(alloc_frugal))]`), no registry to ask, and no frame bus to answer. Those
/// builds paint whole frames on demand, so there is nothing for this to drive and no host loop to
/// keep awake — the honest behaviour is to do nothing, not to carry a stub of a mechanism that
/// does not exist in the profile.
#[cfg(not(alloc_frugal))]
fn advance_host_owned_control(widget: &mut dyn Widget) {
    if !is_host_owned(widget) || widget.manages_own_repaint() {
        return;
    }
    let _ = widget.tick(ANIMATION_FRAME_DELTA_MS);
    let settled = !widget.is_animating();
    crate::widget::runtime::animation_bus_note_host_owned_animating(state_after_advance(settled));
}

/// The frame advance where the profile has no frame bus.
///
/// `mini`/`embedded` have no `widget::runtime`, so there is no registry to distinguish a mounted
/// control from an owned one and no bus to report to. Nothing is advanced, which is what those
/// profiles already did: they repaint whole frames from their own host loop.
#[cfg(alloc_frugal)]
fn advance_host_owned_control(widget: &mut dyn Widget) {
    let _ = widget;
}

/// The delta one frame of the paint path is worth, in milliseconds.
///
/// # Why a constant and not a measured frame time
///
/// At this entry point the crate has no clock and will not grow one: a wrong frame delta is worse
/// than a fixed one — a single long frame (the host loaded a font, the window was occluded) would
/// otherwise teleport every in-flight animation to its end. `Switch::tick` and the other control
/// `tick`s accumulate against a *target* rather than a deadline, so a fixed step simply paces the
/// movement and cannot overshoot. 16 ms is the step a 60 Hz host would use, so an animation takes
/// the same time here as it would on that host.
///
/// # Why it is shared rather than copied
///
/// `Carousel`'s legacy `Event::Timer` arm steps its autoplay clock by one nominal frame, which is
/// the *same* quantity this paint path hands a control. A second `16` there would be a copy of a
/// policy with nothing linking the copy to it — the drift the duration gate exists to stop — so
/// the value is published from here and both readers name it. It is deliberately **not** a
/// `theme.motion` token: it is a sample size, not a duration, which is why
/// `tools/transition_duration_exemptions.txt` records the reason rather than the value being
/// folded into a tempo (BLUE22 §6.7's distinction, and BLUE24 §2.4 gate A).
#[cfg(not(alloc_frugal))]
pub const ANIMATION_FRAME_DELTA_MS: u32 = 16;

/// Reports whether `widget` is a control the frame sweep does **not** own but which is in flight.
///
/// Split out so the two independent facts — "is this mounted?" and "does it owe frames?" — are
/// asked of one named place rather than as a compound condition inside the paint path. The
/// `is_animating()` read is second, so a control that is mounted (the common case) or resting pays
/// nothing for the question.
#[cfg(not(alloc_frugal))]
fn is_host_owned(widget: &dyn Widget) -> bool {
    widget.is_animating() && !crate::widget::runtime::is_mounted(widget.id())
}

/// Maps "has this control settled?" onto the bus fact.
///
/// A named translation rather than a bare negation at the call site: the bus speaks in
/// "a host-owned control is animating", while `tick` speaks in "it needs another frame", and
/// conflating the two is how the two halves of the animation contract drift apart.
#[cfg(not(alloc_frugal))]
fn state_after_advance(settled: bool) -> bool {
    !settled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::MiniToString;
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
