// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Common widget contract implemented by all widget models.

use super::{BaseWidget, WidgetKind};
use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::EventHandler;
use crate::platform::accessibility::AccessibleRole;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::css::CssParser;
use crate::style::{Margin, Padding, WidgetStyle};
use std::any::Any;

/// Common widget contract implemented by all widget models.
pub trait Widget: EventHandler + Any {
    /// Returns shared base widget state for default trait delegation.
    ///
    /// Every concrete widget must override this method (all 167 kinds do). The
    /// default panics instead of silently returning a fake state, so a widget
    /// that forgets the override fails loudly on first use rather than corrupting
    /// geometry/visibility bookkeeping.
    #[track_caller]
    fn base(&self) -> &BaseWidget {
        panic!("Widget::base() not implemented — override in {}", std::any::type_name::<Self>());
    }
    /// Returns mutable base widget state for default trait delegation.
    ///
    /// See [`Widget::base`]; every concrete widget overrides this method.
    #[track_caller]
    fn base_mut(&mut self) -> &mut BaseWidget {
        panic!(
            "Widget::base_mut() not implemented — override in {}",
            std::any::type_name::<Self>()
        );
    }
    /// Get stable widget id.
    fn id(&self) -> ObjectId {
        self.base().id()
    }
    /// Get widget runtime kind.
    fn kind(&self) -> WidgetKind {
        self.base().kind()
    }
    /// Returns the widget's rectangle, in parent-relative logical coordinates.
    ///
    /// The origin is inclusive and the far edge exclusive, matching
    /// [`Rect::contains_point`]. Coordinates are logical (device-independent)
    /// pixels — divide physical pixels by [`Widget::dpi_scale`] before
    /// comparing the two.
    fn geometry(&self) -> Rect {
        self.base().geometry()
    }
    /// Replaces the widget's geometry (origin and size) in a single call.
    ///
    /// Containers call this during layout. Setting geometry does not by itself
    /// request a redraw; call [`Widget::request_redraw`] if the change must be
    /// presented immediately.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base_mut().set_geometry(geometry);
    }
    /// Deprecated alias for [`Widget::geometry`].
    #[deprecated(since = "0.1.0", note = "Use `geometry()` instead.")]
    fn rect(&self) -> Rect {
        self.geometry()
    }
    /// Deprecated alias for [`Widget::set_geometry`].
    #[deprecated(since = "0.1.0", note = "Use `set_geometry()` instead.")]
    fn set_rect(&mut self, rect: Rect) {
        self.set_geometry(rect);
    }
    /// Returns widget position from its geometry origin.
    fn position(&self) -> Point {
        self.geometry().position()
    }
    /// Returns widget size from its geometry extent.
    fn size(&self) -> Size {
        self.geometry().size()
    }
    /// Updates widget position while preserving size.
    fn set_position(&mut self, position: Point) {
        self.set_geometry(Rect::from_position_size(position, self.size()));
    }
    /// Updates widget size while preserving position.
    fn set_size(&mut self, size: Size) {
        self.set_geometry(Rect::from_position_size(self.position(), size));
    }
    /// Returns minimum size constraint when configured.
    fn min_size(&self) -> Option<Size> {
        self.base().min_size()
    }
    /// Returns maximum size constraint when configured.
    fn max_size(&self) -> Option<Size> {
        self.base().max_size()
    }
    /// Sets minimum size constraint.
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.base_mut().set_min_size(min_size);
    }
    /// Sets maximum size constraint.
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.base_mut().set_max_size(max_size);
    }
    /// Returns the parent's object id, or `None` for a root widget.
    ///
    /// This is bookkeeping only: the parent stores a list of child ids in
    /// [`Widget::children`], and the two are kept in sync by
    /// [`Widget::add_child`] / [`Widget::remove_child`].
    fn parent(&self) -> Option<ObjectId> {
        self.base().parent()
    }
    /// Records which widget owns this one, or `None` to detach it. The child
    /// list on the parent is not updated — use [`Widget::add_child`] for that.
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base_mut().set_parent(parent);
    }
    /// Adds a child id to this widget's child list.
    ///
    /// Ids are stored, not widgets: the child is not moved, resized, or
    /// re-parented by this call. Duplicate ids are allowed by the base
    /// implementation, so callers must avoid adding the same child twice.
    fn add_child(&mut self, child: ObjectId) {
        self.base_mut().add_child(child);
    }
    /// Removes a child id from this widget's child list.
    ///
    /// No-op if the id is not present, so removing an already-removed child is
    /// safe.
    fn remove_child(&mut self, child: ObjectId) {
        self.base_mut().remove_child(child);
    }
    /// Returns the ids of this widget's direct children, in insertion order.
    fn children(&self) -> &[ObjectId] {
        self.base().children()
    }
    /// Show widget.
    fn show(&mut self) {
        self.base_mut().show();
    }
    /// Hide widget.
    fn hide(&mut self) {
        self.base_mut().hide();
    }
    /// Returns `true` if the widget is currently shown (see [`Widget::show`]).
    fn is_visible(&self) -> bool {
        self.base().is_visible()
    }
    /// Sets visibility, equivalent to [`Widget::show`] or [`Widget::hide`].
    /// Hidden widgets are skipped during painting and hit testing; visibility
    /// is independent of enabled state.
    fn set_visible(&mut self, visible: bool) {
        if visible {
            self.show();
        } else {
            self.hide();
        }
    }
    /// Enables or disables the widget.
    ///
    /// Disabling does not hide the widget: it stays painted but is skipped
    /// when events are delivered, and focusable controls remain in the tab
    /// order (see [`Widget::is_focusable`]).
    fn set_enabled(&mut self, enabled: bool) {
        self.base_mut().set_enabled(enabled);
    }
    /// Returns `true` if the widget accepts input (enabled), `false` if not.
    fn is_enabled(&self) -> bool {
        self.base().is_enabled()
    }
    /// Sets the hover tooltip text for this widget; an empty string clears it.
    fn set_tooltip(&mut self, tooltip: String) {
        self.base_mut().set_tooltip(crate::compat::mini_string_from(tooltip));
    }
    /// Returns the current tooltip text, or `""` when none was set.
    fn tooltip(&self) -> &str {
        self.base().tooltip()
    }
    /// Returns a human-readable accessibility name used by assistive technologies.
    ///
    /// Default behavior prefers tooltip text when present, then falls back to the
    /// widget kind so every widget has a stable non-empty label.
    /// Returns the name assistive technology announces for this widget.
    ///
    /// # Order of preference
    ///
    /// 1. The tooltip, when the host set one: it is an explicit description written
    ///    for this control, so it says more than a label does.
    /// 2. The control's own label (`text` / `title` / `message`), which is what a
    ///    sighted user reads and therefore what a screen-reader user expects.
    /// 3. The kind name, as a last resort so the result is never empty.
    ///
    /// Step 2 is what changed: skipping it made every labelled control announce
    /// itself as `"Button"` / `"Label"` — the kind, not the control. The label is
    /// read through the property contract rather than a per-kind table, so a control
    /// added later is announced correctly without touching this method.
    fn accessible_name(&self) -> String {
        let tooltip = self.tooltip().trim();
        if !tooltip.is_empty() {
            return tooltip.to_string();
        }
        // Read the label through the property contract rather than a per-kind
        // table, so a control added later is announced correctly without editing
        // this method. A control with no contract simply skips to the fallback.
        if let Some(props) = self.properties_dyn() {
            for property in crate::control_backend::custom::LABEL_PROPERTY_NAMES {
                if let Ok(crate::widget::capability::CapabilityValue::String(label)) =
                    props.get(property)
                {
                    if !label.trim().is_empty() {
                        return label;
                    }
                }
            }
        }
        format!("{:?}", self.kind())
    }
    /// Returns the semantic accessibility role for this widget.
    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::from(self.kind())
    }
    /// Returns a short accessibility description with current visibility/enabled state.
    fn accessible_description(&self) -> String {
        let mut state_flags: Vec<&str> = Vec::new();
        if !self.is_enabled() {
            state_flags.push("disabled");
        }
        if !self.is_visible() {
            state_flags.push("hidden");
        }
        if state_flags.is_empty() {
            format!("{:?}", self.accessible_role())
        } else {
            format!("{:?} ({})", self.accessible_role(), state_flags.join(", "))
        }
    }
    /// Returns the device pixel ratio applied to this widget.
    ///
    /// `1.0` is the 96-DPI baseline; `2.0` on a HiDPI display. Logical
    /// coordinates (geometry, hit testing) are scaled by this when rasterised,
    /// so callers should not pre-multiply their own sizes by it.
    fn dpi_scale(&self) -> f32 {
        self.base().dpi_scale()
    }

    /// Returns this widget as a [`crate::widget::draw::Draw`] implementor, when it paints itself.
    ///
    /// # Why this exists
    ///
    /// [`Widget`] does not require [`crate::widget::draw::Draw`], because many widgets delegate to a
    /// real OS control. But the rendering pipeline holds widgets as
    /// `&mut dyn Widget` and still needs to ask "can *you* paint yourself?".
    /// Without this bridge the only way to reach `Draw::draw` was a concrete
    /// generic bound (`W: Draw + Widget`), which no `Box<dyn Widget>` satisfies —
    /// so self-drawn widgets could never be painted generically, and mounting
    /// one into a native window produced an empty surface.
    ///
    /// # Contract
    ///
    /// Return `Some(self)` from every widget that implements [`crate::widget::draw::Draw`]. The
    /// default returns `None`, which is the honest answer for OS-backed widgets
    /// and keeps existing implementors compiling unchanged.
    ///
    /// The example uses `Button` because it is part of the widget set that every
    /// profile compiles: a doctest naming a `full_widgets`-only control would fail
    /// to compile under `mini`/`embedded`, and a doc example that only holds on one
    /// profile is a liability (see the profile alias in `build.rs`).
    ///
    /// ```
    /// use rust_widgets::core::Rect;
    /// use rust_widgets::widget::base_widgets::button::Button;
    /// use rust_widgets::widget::{Draw, Widget};
    ///
    /// let mut button = Button::new("OK".to_string(), Rect::new(0, 0, 80, 24));
    /// assert!(button.as_draw_mut().is_some());
    ///
    /// let widget: &mut dyn Widget = &mut button;
    /// assert!(widget.as_draw_mut().is_some());
    /// ```
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        None
    }

    /// Returns this widget as its property contract, when it implements one.
    ///
    /// # Why this hook exists
    ///
    /// [`crate::widget::capability::properties_trait::WidgetProperties`] is
    /// implemented on the **concrete** control, because that is the only place its
    /// fields are visible. The reflection entry points, however, hold
    /// `&dyn Widget`, so they need a way back to the concrete type. `Widget: Any`
    /// provides the downcast; this method is where the widget answers with itself.
    ///
    /// A control that implements `WidgetProperties` overrides this with
    /// `Some(self)`. Returning `None` (the default) means "this control has not
    /// declared a contract", which the caller reports as
    /// `UnsupportedOnWidget` — an honest answer that lets the migration proceed
    /// control by control without a flag day.
    ///
    /// Implementing it is one line, and a control that declares properties but
    /// forgets the override is caught by
    /// `capability::properties_trait`'s coverage test rather than silently
    /// answering nothing.
    fn properties_dyn(
        &self,
    ) -> Option<&dyn crate::widget::capability::properties_trait::WidgetProperties> {
        None
    }

    /// Mutable counterpart to [`Widget::properties_dyn`].
    fn properties_dyn_mut(
        &mut self,
    ) -> Option<&mut dyn crate::widget::capability::properties_trait::WidgetProperties> {
        None
    }
    /// Overrides the device pixel ratio for this widget.
    ///
    /// Non-positive or non-finite values are rejected by the base
    /// implementation, which keeps the previous scale rather than producing
    /// degenerate geometry.
    fn set_dpi_scale(&mut self, scale: f32) {
        self.base_mut().set_dpi_scale(scale);
    }
    /// Treats `key` as a translation-key lookup for the tooltip instead of
    /// literal text, so the displayed tooltip follows the active locale.
    fn set_translated_tooltip(&mut self, key: &str) {
        self.base_mut().set_translated_tooltip(key);
    }
    /// Returns the full style record (colors, borders, padding, margin, font)
    /// backing all the style shorthand accessors on this trait.
    fn style(&self) -> &WidgetStyle {
        self.base().style()
    }
    /// Replaces the whole style record at once, overwriting every style field.
    /// Prefer the individual shorthand setters when only one property changes.
    fn set_style(&mut self, style: WidgetStyle) {
        self.base_mut().set_style(style);
    }
    /// Returns optional background color shorthand.
    fn background_color(&self) -> Option<Color> {
        self.style().background_color
    }
    /// Sets optional background color shorthand.
    fn set_background_color(&mut self, color: Option<Color>) {
        self.base_mut().style_mut().background_color = color;
    }
    /// Returns optional foreground (text) color shorthand.
    fn foreground_color(&self) -> Option<Color> {
        self.style().text_color
    }
    /// Sets optional foreground (text) color shorthand.
    fn set_foreground_color(&mut self, color: Option<Color>) {
        self.base_mut().style_mut().text_color = color;
    }
    /// Returns optional font shorthand.
    fn font(&self) -> Option<&Font> {
        self.style().font.as_ref()
    }
    /// Sets optional font shorthand.
    fn set_font(&mut self, font: Option<Font>) {
        self.base_mut().style_mut().font = font;
    }
    /// Returns optional border color shorthand.
    /// Returns the border colour, or `None` when the widget draws no border
    /// (the theme default is then used by the renderer).
    fn border_color(&self) -> Option<Color> {
        self.style().border_color
    }
    /// Returns the border stroke width in logical pixels, or `None` for the
    /// theme default. A width of `0` is distinct from `None`: it means a
    /// border that is explicitly drawn with no thickness.
    fn border_width(&self) -> Option<u32> {
        self.style().border_width
    }
    /// Returns the corner rounding radius in logical pixels, or `None` for the
    /// theme default. The value is clamped by the renderer to half the shorter
    /// side of the widget's rectangle.
    fn border_radius(&self) -> Option<u32> {
        self.style().border_radius
    }
    /// Sets optional border color shorthand.
    fn set_border_color(&mut self, color: Option<Color>) {
        self.base_mut().style_mut().border_color = color;
    }
    /// Sets border width shorthand.
    fn set_border_width(&mut self, width: u32) {
        self.base_mut().style_mut().border_width = Some(width);
    }
    /// Sets border radius shorthand.
    fn set_border_radius(&mut self, radius: u32) {
        self.base_mut().style_mut().border_radius = Some(radius);
    }
    /// Sets border shorthand in one call.
    /// Sets the border colour, width, and radius in one style update.
    ///
    /// `width` and `radius` are logical pixels; passing `None` for `color`
    /// clears the explicit border colour and falls back to the theme. Use
    /// `0` for a borderless widget rather than `None` if you want to override
    /// the theme explicitly.
    fn set_border(&mut self, color: Option<Color>, width: u32, radius: u32) {
        let mut style = self.style().clone();
        style.border_color = color;
        style.border_width = Some(width);
        style.border_radius = Some(radius);
        self.set_style(style);
    }
    /// Returns current per-side content padding.
    fn padding(&self) -> &Padding {
        &self.style().padding
    }
    /// Returns current per-side outer margin.
    fn margin(&self) -> &Margin {
        &self.style().margin
    }
    /// Updates widget content padding while preserving other style properties.
    fn set_padding(&mut self, padding: Padding) {
        self.base_mut().style_mut().padding = padding;
    }
    /// Updates widget margin while preserving other style properties.
    fn set_margin(&mut self, margin: Margin) {
        self.base_mut().style_mut().margin = margin;
    }
    /// Returns the connection scope whose lifetime is tied to this widget.
    ///
    /// Slots connected through this scope are disconnected automatically when
    /// the widget is dropped, which is the recommended way to avoid callbacks
    /// firing into freed state. Pass it as the `owner` to
    /// [`Signal::connect_scoped`](crate::signal::Signal::connect_scoped) rather
    /// than connecting with a bare signal handle.
    fn connection_scope(&self) -> &ConnectionScope {
        self.base().connection_scope()
    }
    /// Optional clicked signal (legacy API compatibility).
    ///
    /// The signal is present on every widget but is only emitted by widgets
    /// that are clickable; a widget that never emits it is normal, not a bug.
    /// Prefer the typed interaction signals below for new code.
    fn clicked_signal(&self) -> &GenericSignal {
        &self.base().clicked
    }
    /// Optional changed signal (legacy API compatibility).
    /// Emitted when a stateful value changes (e.g., slider value, checkbox state,
    /// line edit text). Concrete widgets with changeable state should wire their
    /// own value-change emission to `self.base_mut().changed.emit()`.
    fn changed_signal(&self) -> &GenericSignal {
        &self.base().changed
    }
    /// Emits on hover/move interactions while pointer is over widget.
    ///
    /// The payload is the pointer position in parent-relative coordinates.
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base().hover_signal()
    }
    /// Emits on mouse/pointer press interactions.
    ///
    /// Payload is `(position_in_parent_coordinates, pointer_button)`; button
    /// numbering is backend-defined, with `0` as the primary button everywhere.
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base().mouse_down_signal()
    }
    /// Emits on mouse/pointer release interactions, with the same
    /// `(position, button)` payload as [`Widget::mouse_down_signal`].
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base().mouse_up_signal()
    }
    /// Emits on keyboard press interactions.
    ///
    /// Payload is `(key_code, modifiers)`. Modifier bit assignments are
    /// backend-defined, not fixed by this crate.
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base().key_down_signal()
    }
    /// Emits on keyboard release interactions, with the same
    /// `(key_code, modifiers)` payload as [`Widget::key_down_signal`].
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base().key_up_signal()
    }
    /// Emits when logical focus is gained.
    ///
    /// Focus is granted by the runtime's focus router; a widget cannot emit
    /// this for itself.
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base().focus_gained_signal()
    }
    /// Emits when logical focus is lost.
    ///
    /// Carries no payload: the losing widget is the receiver, and the gaining
    /// widget announces itself through its own `focus_gained_signal`.
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base().focus_lost_signal()
    }
    /// Emits when redraw is requested.
    /// Returns the redraw-request signal used by [`Widget::request_redraw`].
    ///
    /// Emitted when the widget needs to be repainted. It carries no payload:
    /// the receiving runtime re-queries all widget state, so emitting it more
    /// often than strictly necessary is safe but wasteful.
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base().redraw_requested_signal()
    }
    /// Returns the layout-request signal used by [`Widget::request_layout`].
    ///
    /// Emitted when the widget's preferred size may have changed and a layout
    /// pass is required. A layout request implies the widget will also need a
    /// redraw, but the two signals are delivered independently.
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base().layout_requested_signal()
    }
    /// Whether this widget has ever asked to be repainted.
    ///
    /// Delegates to [`BaseWidget::has_ever_requested_redraw`]. Used by the runtime's
    /// repaint auto-decision to leave a surface that nothing redraws in
    /// `RepaintMode::Full` rather than making it pay for damage tracking.
    fn has_ever_requested_redraw(&self) -> bool {
        self.base().has_ever_requested_redraw()
    }
    /// Requests redraw and emits redraw signal.
    ///
    /// Routes through [`BaseWidget::request_redraw`] rather than emitting the signal
    /// itself, so the one place that records damage is the one place every call site
    /// reaches. This default and the inherent method used to emit independently, which
    /// meant `mark_widget_damage` saw only the calls that happened to name the inherent
    /// one — partial repaint then worked or not depending on which spelling a call site
    /// used, and no test could tell the two apart because both emitted the same signal.
    fn request_redraw(&self) {
        self.base().request_redraw();
    }
    /// Requests layout and emits layout signal.
    fn request_layout(&self) {
        self.layout_requested_signal().emit();
    }
    /// Returns the preferred size hint for layout calculations.
    ///
    /// The default implementation returns `Size::new(0, 0)` (unknown/unconstrained).
    /// Widgets should override this to provide meaningful content-based size hints
    /// so that layout containers can properly allocate space.
    fn size_hint(&self) -> Size {
        Size::new(0, 0)
    }

    /// Apply CSS styles to this widget. The `css` text is parsed and rules matching
    /// the widget's kind and optional class/id are applied to the widget's style.
    fn apply_css(&mut self, css: &str, class: Option<&str>) -> Result<(), String> {
        let kind_str = format!("{:?}", self.kind());
        let mut style = self.style().clone();
        CssParser::parse_and_apply(css, &kind_str, class, None, None, &mut style)?;
        self.set_style(style);
        Ok(())
    }

    /// Checks whether the given point falls within this widget's interactive area.
    ///
    /// By default, this respects the touch-target expansion set via
    /// `WidgetStyle::with_touch_target()`, so that small widgets remain
    /// easily tappable on touch devices.
    fn contains_point(&self, point: Point) -> bool {
        self.base().contains_point_with_touch_expansion(point)
    }

    /// Whether keyboard focus can be given to this widget.
    ///
    /// Focusable controls are the ones a user can reach by pressing Tab, and the
    /// ones that receive key events. The registry asks each widget as it mounts
    /// (see `crate::widget::runtime::register`), so a control declares this itself
    /// rather than being looked up in a kind table — a third-party widget the
    /// library has never heard of joins the tab order by answering `true`.
    ///
    /// The default is `false`: a control that has not thought about focus should not
    /// silently swallow the Tab key. Enabled state is *not* consulted here — a
    /// disabled-but-focusable control must stay in the tab order so a user can move
    /// past it (the router skips disabled controls when delivering, not when
    /// ordering).
    fn is_focusable(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::Widget;
    use crate::core::Rect;
    use crate::widget::base_widgets::button::Button;

    #[test]
    fn widget_accessible_name_prefers_the_label_then_the_tooltip() {
        let mut button = Button::new("Open".to_string(), Rect::new(0, 0, 100, 32));
        // The control's own label is what a screen-reader user expects to hear, so
        // it wins over the kind name. Reporting `"Button"` for a labelled control
        // was the old behaviour and told the user nothing about which button it is.
        assert_eq!(button.accessible_name(), "Open");

        // An explicit tooltip is a better description than the label, so it wins
        // when present.
        button.set_tooltip("Open file".to_string());
        assert_eq!(button.accessible_name(), "Open file");
    }

    /// An unlabelled control still gets a non-empty name.
    #[test]
    fn widget_accessible_name_falls_back_to_the_kind() {
        let button = Button::new(String::new(), Rect::new(0, 0, 100, 32));
        assert_eq!(button.accessible_name(), "Button");
    }

    #[test]
    fn widget_accessible_description_reflects_state_flags() {
        let mut button = Button::new("Open".to_string(), Rect::new(0, 0, 100, 32));
        assert_eq!(button.accessible_description(), "Button");

        button.set_enabled(false);
        button.hide();
        assert_eq!(button.accessible_description(), "Button (disabled, hidden)");
    }

    #[test]
    fn widget_structure_button_exposes_expected_kind_and_geometry() {
        let button = Button::new("Run".to_string(), Rect::new(10, 20, 120, 36));
        assert_eq!(button.kind(), crate::widget::WidgetKind::Button);
        assert_eq!(button.geometry(), Rect::new(10, 20, 120, 36));
    }

    #[test]
    fn widget_structure_button_has_distinct_object_ids() {
        let a = Button::new("A".to_string(), Rect::new(0, 0, 80, 24));
        let b = Button::new("B".to_string(), Rect::new(0, 0, 80, 24));
        assert_ne!(a.id(), b.id());
    }
}
