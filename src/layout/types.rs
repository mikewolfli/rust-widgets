// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Space allocation preference used by layout items.

use crate::compat::{vec, Any, Vec};
use crate::core::{ObjectId, Point, Rect, Size};
use crate::layout::hints::ChildInfo;
/// How a layout item reacts to the space its parent offers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizePolicy {
    /// Use fixed size defined by constraints.
    Fixed,
    /// Hug the natural size stated by the item's constraints and never take leftover room.
    ///
    /// Distinct from [`Self::Expanding`] (LAY-002): only the strategies that ask for room
    /// participate in the distribution of the space left over after every item's floor is
    /// paid; a `Preferred` item keeps the extent its constraints name and may only be
    /// scaled down when the parent cannot pay for it.
    Preferred,
    /// Expand to consume remaining space.
    Expanding,
    /// Never shrink below the natural size, but may grow.
    Minimum,
    /// Never grow beyond the natural size, but may shrink.
    Maximum,
    /// Like [`Self::Expanding`], but the natural size is a hard floor.
    MinimumExpanding,
    /// The widget's natural size is ignored and the layout owns the extent.
    Ignored,
}
/// Min/max limits applied during layout calculation.
///
/// # Adapter, not the source of truth (LAY-N03 / G-401)
///
/// The crate grew three answers to "how much room does this child want": this type plus
/// [`SizePolicy`] (box layout), [`Hints`](crate::layout::Hints) (the hint-aware `arrange`
/// channel), and `FlexItem`'s own min/max. `Hints` is the converged form: it carries both
/// axes, and it is what `CompositeWidget` feeds every layout through `ChildInfo`.
///
/// These two remain because a caller that builds a `BoxLayout` directly has no `ChildInfo`
/// to supply — they are that layout's adapter. New layouts consume `Hints`/`ChildInfo` only;
/// this type is retained as a construction alias with a conversion into the converged form
/// ([`Self::to_axis_hints`]). Removing it would break the programmatic API for no behavioural
/// gain; keeping it undocumented as an adapter is how the three vocabularies drift again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutConstraints {
    /// Minimum major-axis size.
    pub min: u32,
    /// Optional maximum major-axis size.
    pub max: Option<u32>,
}
impl LayoutConstraints {
    /// Creates new layout constraints.
    pub fn new(min: u32, max: Option<u32>) -> Self {
        Self { min, max }
    }

    /// This adapter expressed in the single size vocabulary, [`AxisHints`](crate::layout::AxisHints).
    ///
    /// `min` is both the floor and the preferred value (the adapter has no separate preferred
    /// size), and a missing maximum is the unbounded hint. This is the conversion a new layout
    /// uses when a caller still hands it legacy constraints.
    pub fn to_axis_hints(self) -> crate::layout::AxisHints {
        crate::layout::AxisHints::new(self.min, self.min, self.max.unwrap_or(u32::MAX))
    }
}

impl SizePolicy {
    /// This policy expressed as [`LayoutParams`](crate::layout::LayoutParams), the converged
    /// "how should the parent treat me" side of the hint vocabulary.
    ///
    /// Only the policies that ask for leftover room set `fill`; the rest keep the default
    /// (preferred-or-smaller) treatment. `stretch` stays `1`, matching the box layout's
    /// default weight, because the policy does not carry a weight.
    pub fn to_layout_params(self) -> crate::layout::LayoutParams {
        let fill = matches!(self, Self::Expanding | Self::MinimumExpanding | Self::Ignored);
        if fill {
            crate::layout::LayoutParams::stretched(1)
        } else {
            crate::layout::LayoutParams::new()
        }
    }
}
/// Layout context carrying device adaptation parameters.
///
/// Passed to [`Layout::update_with_context`] to enable DPI-aware
/// spacing, margins, and minimum touch-target sizes.
#[derive(Debug, Clone, Copy)]
pub struct LayoutContext {
    /// Scale factor applied to spacing, margins, and padding.
    /// Derived from device DPI and font scale factors.
    pub layout_scale: f32,
    /// Scale factor applied to font/metric sizes.
    ///
    /// # Why the default is not a literal
    ///
    /// A hardcoded `1.0` here meant this field could not express a device's text-scale
    /// preference at all: a caller building a context from `Default::default()` got "no scaling"
    /// on hardware that had asked for scaling. The default now reads the platform's text-scale
    /// report (`platform::profile::text_scale`), so a context a caller does not customise already
    /// reflects the device it is running on.
    pub font_scale: f32,
    /// Minimum touch-target size in logical pixels.
    ///
    /// Defaults to the **device class's** recommended minimum (`TouchTargetSize::dimensions()`),
    /// not a fixed 32×32: the value is a platform fact, and a phone context that claimed a desktop
    /// minimum would let a layout place controls closer together than a finger can address.
    pub min_touch_size: Size,
}
/// Applies the context's `min_touch_size` to a child rectangle.
///
/// # Why a layout grows a child rather than leaving it alone
///
/// `LayoutContext::min_touch_size` is a platform fact — the smallest area a finger can
/// reliably address on this device class — and a layout is the layer that decides how much
/// room each child gets. A layout that ignores it places controls closer together than the
/// hardware can address, and no amount of hit-test expansion recovers a target the *neighbouring
/// control* is drawn on top of.
///
/// The growth is centred, so a control stays where the layout put it and gains reach on both
/// sides; and it is **clamped to the parent's rectangle at the caller**, because a child that
/// grew past its container would be clipped there instead of being reachable.
///
/// This deliberately does not shrink anything. A layout that has less space than the minimum
/// returns the rectangle it computed; the alternative would be to make a control smaller than
/// its own content to satisfy a floor, which trades one unusable control for another.
pub fn grow_to_min_touch_size(child: Rect, min_touch_size: Size) -> Rect {
    if child.width >= min_touch_size.width && child.height >= min_touch_size.height {
        return child;
    }
    let width = child.width.max(min_touch_size.width);
    let height = child.height.max(min_touch_size.height);
    Rect::new(
        child.x - (width - child.width) as i32 / 2,
        child.y - (height - child.height) as i32 / 2,
        width,
        height,
    )
}

impl Default for LayoutContext {
    fn default() -> Self {
        Self {
            layout_scale: 1.0,
            font_scale: crate::platform::profile::text_scale(),
            min_touch_size: crate::platform::profile::recommended_touch_target().dimensions(),
        }
    }
}

/// Common interface implemented by all layout managers.
pub trait Layout {
    /// The smallest size this layout needs to place its children.
    ///
    /// The trait used to have no upward channel at all, so a window's minimum size could
    /// only be set by the platform. A layout that knows its children's floors answers
    /// here; the default is zero, which preserves every existing implementor.
    fn minimum_size(&self) -> Size {
        Size::new(0, 0)
    }

    /// The preferred size this layout wants for its children.
    ///
    /// The default is zero ("no opinion"), which keeps layouts that have not adopted the
    /// channel behaving exactly as before.
    fn size_hint(&self) -> Size {
        Size::new(0, 0)
    }

    /// Whether the layout would consume extra room on each axis.
    ///
    /// `(horizontal, vertical)`. The default `(false, false)` is the conservative answer;
    /// a box layout overrides it with its own axis.
    fn expanding_directions(&self) -> (bool, bool) {
        (false, false)
    }

    /// Discards any cached arrangement.
    ///
    /// A layout that caches (the flex solver does) overrides this; the default is a no-op
    /// because a stateless layout has nothing to discard.
    fn invalidate(&mut self) {}

    /// The height this layout needs for a given width.
    ///
    /// The default returns `height` unchanged, which is the "no height-for-width
    /// negotiation" behaviour every existing layout had. A wrapping layout overrides it.
    fn height_for_width(&self, _width: u32, height: u32) -> u32 {
        height
    }

    /// Adds widget into layout with optional stretch factor.
    fn add_widget(&mut self, widget_id: ObjectId, stretch: u32);
    /// Remove widget from layout.
    fn remove_widget(&mut self, widget_id: ObjectId);
    /// Recompute child geometries within given rect.
    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));

    /// Recompute child geometries, given what each child wants.
    ///
    /// # Why this exists alongside [`Layout::update`]
    ///
    /// `update` can only **write** child geometry. It receives `ObjectId`s, not widgets,
    /// so it cannot ask "how big does this child want to be?" — which is why the layouts
    /// that needed sizes had to be told in advance
    /// ([`FlexLayout::set_child_sizes`](crate::layout::FlexLayout::set_child_sizes),
    /// and the same workaround in `wrap` and `absolute`). This method is the channel
    /// that lets a layout do the asking, as React, SwiftUI and the platform toolkits all do.
    ///
    /// # Why there is no default implementation
    ///
    /// The default used to forward to [`Layout::update`], which made the fallback chain
    /// silent: a wrapper that forgot to forward `arrange` to its inner layout kept
    /// "working" while every `ChildInfo` the caller passed — sizing hints, margins,
    /// fill/stretch — was dropped on the floor (G-403). With no default, a wrapper that
    /// misses the entry fails to compile; [`impl_layout_forward!`] generates the four
    /// forwards for wrappers that do forward, and each solver states explicitly what it
    /// does with the hint channel.
    fn arrange(&self, rect: Rect, children: &[ChildInfo], out: &mut dyn FnMut(ObjectId, Rect));

    /// [`arrange`](Self::arrange) with the device context available to it.
    ///
    /// # Why this exists rather than a context on `arrange`
    ///
    /// `arrange` is implemented by several layouts and called from a dozen places; widening its
    /// signature would be the "76 mechanical migrations" shape this crate has already paid for
    /// twice, and the context is only *needed* by the layouts that scale (and by the composite
    /// builder, which applies the touch floor). A default body forwards to `arrange`, so every
    /// existing implementation and caller is untouched and gets exactly its current behaviour.
    ///
    /// # Why there is no default
    ///
    /// The default forwarded to [`Layout::arrange`] with the context dropped, which is
    /// exactly the failure mode G-403 records: a wrapper that skipped this entry silently
    /// lost the device scale and the touch floor on the context-aware path. Requiring the
    /// entry turns that silent loss into a compile error; [`impl_layout_forward!`]
    /// generates the forward for wrappers that do forward.
    fn arrange_with_context(
        &self,
        rect: Rect,
        children: &[ChildInfo],
        context: &LayoutContext,
        out: &mut dyn FnMut(ObjectId, Rect),
    );

    /// Recompute child geometries from explicit position/size primitives.
    fn update_from_position_size(
        &self,
        position: Point,
        size: Size,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    ) {
        self.update(Rect::from_position_size(position, size), widgets);
    }
    /// Returns all child widget IDs managed by this layout.
    fn child_ids(&self) -> Vec<ObjectId> {
        vec![]
    }

    /// Returns true if the given widget ID is a child of this layout.
    fn has_child(&self, _id: ObjectId) -> bool {
        false
    }

    /// Number of child widgets managed by this layout.
    ///
    /// The default counts [`child_ids`](Self::child_ids); a layout that can answer without
    /// materialising the id vector should override it.
    fn widget_count(&self) -> usize {
        self.child_ids().len()
    }

    /// Position of `id` in [`child_ids`](Self::child_ids) order.
    ///
    /// This is the index the child is addressed by in [`update`](Self::update)'s emission
    /// order, not necessarily an index into a layout-specific internal list. Returns `None`
    /// when the layout does not manage `id`.
    fn index_of(&self, id: ObjectId) -> Option<usize> {
        self.child_ids().iter().position(|child| *child == id)
    }

    /// The rectangle `id` would receive inside `rect`.
    ///
    /// A single-point query for a caller that needs one child's geometry: the default runs
    /// [`update`](Self::update) and captures the matching emission, which is the only channel
    /// the trait has for child rectangles. **This is the full-solve fallback**: a layout that
    /// can derive one child's rectangle from its own cell/track model (grid, absolute, box)
    /// overrides this, because a hit test that asks about one child must not pay for every
    /// child (G-158). Returns `None` when the layout does not place `id`.
    fn child_rect(&self, id: ObjectId, rect: Rect) -> Option<Rect> {
        let mut found = None;
        self.update(rect, &mut |child, child_rect| {
            if child == id && found.is_none() {
                found = Some(child_rect);
            }
        });
        found
    }

    /// Removes all children from this layout.
    /// Default implementation does nothing (layouts without children).
    fn clear(&mut self) {
        // Default: layouts that don't track children externally are no-ops.
    }

    /// Update child geometries with device-aware scaling context.
    ///
    /// # Why there is no default
    ///
    /// The default ignored the context and delegated to [`Layout::update`], so a wrapper
    /// that forgot to forward this entry silently laid out with nominal spacing — no
    /// `layout_scale`, no `font_scale`, no touch floor (G-403). With the entry required
    /// the miss is a compile error, and [`impl_layout_forward!`] generates the forward
    /// for wrappers that do forward.
    fn update_with_context(
        &self,
        rect: Rect,
        context: &LayoutContext,
        widgets: &mut dyn FnMut(ObjectId, Rect),
    );

    /// Enables downcasting from `dyn Layout` to concrete types.
    /// Required by the layout inspector for introspection.
    fn as_any(&self) -> &dyn Any;

    /// Enables mutable downcasting from `dyn Layout` to concrete types.
    /// Required for mutation access to concrete layout implementations
    /// through the trait object.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Generates the four execution entries of [`Layout`] for a wrapper layout that
/// forwards to an inner layout.
///
/// G-403: the trait's four execution entries (`update`, `update_with_context`,
/// `arrange`, `arrange_with_context`) have no defaults, so a wrapper that misses one
/// fails to compile instead of silently falling back to `update` and dropping the hint
/// channel (`ChildInfo`) and the device context. This macro is the one place the
/// forwarding body is written.
///
/// The first argument is the wrapper's field holding the inner layout. The generated
/// entries forward to it through `self.<field>`; the wrapper still implements
/// `add_widget`, `remove_widget`, `as_any` and the rest itself.
///
/// The one-argument form forwards the rectangle unchanged:
///
/// ```ignore
/// impl Layout for Wrapper {
///     impl_layout_forward!(inner);
/// }
/// ```
///
/// The two-argument form passes the rectangle through the named inherent method of the
/// wrapper first — `self.<adjust>(rect) -> Rect` — for wrappers that transform the
/// geometry before their inner layout sees it (a `KeyboardAwareLayout` shifts it by the
/// keyboard offset):
///
/// ```ignore
/// impl Layout for Wrapper {
///     impl_layout_forward!(inner, adjust_rect);
/// }
/// ```
#[macro_export]
macro_rules! impl_layout_forward {
    ($field:ident) => {
        fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
            self.$field.update(rect, widgets);
        }
        fn update_with_context(
            &self,
            rect: Rect,
            context: &LayoutContext,
            widgets: &mut dyn FnMut(ObjectId, Rect),
        ) {
            self.$field.update_with_context(rect, context, widgets);
        }
        fn arrange(&self, rect: Rect, children: &[ChildInfo], out: &mut dyn FnMut(ObjectId, Rect)) {
            self.$field.arrange(rect, children, out);
        }
        fn arrange_with_context(
            &self,
            rect: Rect,
            children: &[ChildInfo],
            context: &LayoutContext,
            out: &mut dyn FnMut(ObjectId, Rect),
        ) {
            self.$field.arrange_with_context(rect, children, context, out);
        }
    };
    ($field:ident, $adjust:ident) => {
        fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
            let rect = self.$adjust(rect);
            self.$field.update(rect, widgets);
        }
        fn update_with_context(
            &self,
            rect: Rect,
            context: &LayoutContext,
            widgets: &mut dyn FnMut(ObjectId, Rect),
        ) {
            let rect = self.$adjust(rect);
            self.$field.update_with_context(rect, context, widgets);
        }
        fn arrange(&self, rect: Rect, children: &[ChildInfo], out: &mut dyn FnMut(ObjectId, Rect)) {
            let rect = self.$adjust(rect);
            self.$field.arrange(rect, children, out);
        }
        fn arrange_with_context(
            &self,
            rect: Rect,
            children: &[ChildInfo],
            context: &LayoutContext,
            out: &mut dyn FnMut(ObjectId, Rect),
        ) {
            let rect = self.$adjust(rect);
            self.$field.arrange_with_context(rect, children, context, out);
        }
    };
}
