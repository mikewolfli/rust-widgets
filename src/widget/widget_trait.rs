// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Common widget contract implemented by all widget models.

use super::{BaseWidget, WidgetKind};
use crate::compat::{format, Any, String, ToString, Vec};
use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::EventHandler;
use crate::platform::accessibility::AccessibleRole;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::css::CssParser;
use crate::style::{Margin, Padding, WidgetStyle};

/// Common widget contract implemented by all widget models.
pub trait Widget: EventHandler + Any {
    /// Returns shared base widget state for default trait delegation.
    ///
    /// Every concrete widget must override this method (all `WidgetKind` variants do). The
    /// default panics instead of silently returning a fake state, so a widget
    /// that forgets the override fails loudly on first use rather than corrupting
    /// geometry/visibility bookkeeping.
    #[track_caller]
    fn base(&self) -> &BaseWidget {
        panic!("Widget::base() not implemented — override in {}", core::any::type_name::<Self>());
    }
    /// Returns mutable base widget state for default trait delegation.
    ///
    /// See [`Widget::base`]; every concrete widget overrides this method.
    #[track_caller]
    fn base_mut(&mut self) -> &mut BaseWidget {
        panic!(
            "Widget::base_mut() not implemented — override in {}",
            core::any::type_name::<Self>()
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
        //
        // The *choice* of property is `widget_label_property_name`'s, so the name
        // announced here is the one the constructor helper writes and the backend
        // reads — three call sites cannot drift about which spelling is the label.
        if let Some(props) = self.properties_dyn() {
            if let Some(property) =
                crate::control_backend::custom::widget_label_property_name(props)
            {
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
    /// Returns the value assistive technology announces for this widget, or an empty string.
    ///
    /// # Why this is separate from the name
    ///
    /// A name and a value are different facts, and a screen reader announces both: "Volume,
    /// slider, 40 percent" is name + role + value. Before this existed every control announced only
    /// its name, so a slider, a progress bar and a rating were indistinguishable to assistive
    /// technology — a user could hear *what* the control was but never *where it stood*, which is
    /// the one thing those controls exist to communicate.
    ///
    /// # How it resolves
    ///
    /// Through the property contract, so a control added later is announced correctly without
    /// editing this method and a control that has no value concept simply answers empty rather than
    /// a fabricated `0`. The spellings are [`VALUE_PROPERTY_NAMES`]'s decision, made once.
    fn accessible_value(&self) -> String {
        // A disabled control's value is still its value: a reader inspecting a greyed-out slider
        // benefits from knowing where it sits, and suppressing it would make "disabled" and
        // "valueless" announce identically.
        if let Some(props) = self.properties_dyn() {
            if let Some(property) =
                crate::control_backend::custom::widget_value_property_name(props)
            {
                if let Ok(value) = props.get(property) {
                    return value.to_announcement_string();
                }
            }
        }
        String::new()
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

    /// The [`WidgetState`] this control is currently in.
    ///
    /// # Why this is on the trait and not on each control
    ///
    /// A theme may describe `"button:hover"`, and `THEME_STATE` keys were already being resolved by
    /// `ThemeManager::resolve_style_for_state` — but nothing ever *asked* a control what state it was
    /// in, so every such override was unreachable. The mechanism existed end to end except for the
    /// one argument in the middle.
    ///
    /// The default here answers the two states every control shares (disabled, then resting), which
    /// is enough for `"<kind>:disabled"` to work everywhere without each control implementing
    /// anything. A control with richer state overrides this — a button adds hover and pressed, a
    /// toggle adds checked.
    ///
    /// # Why a single state rather than a set
    ///
    /// Flutter models `WidgetState` as a `Set` because several states genuinely hold at once
    /// (`focused | hovered`). Encoding that here would change this type's public shape, which rule
    /// #21 forbids doing silently; the trait method is the additive step. A control that has several
    /// states true at once reports the one with the strongest visual claim, in this order: disabled
    /// (the control is inert), pressed (an active gesture), checked/selected (a persistent fact),
    /// hovered, then resting.
    fn widget_state(&self) -> crate::style::WidgetState {
        if self.is_enabled() {
            crate::style::WidgetState::Normal
        } else {
            crate::style::WidgetState::Disabled
        }
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
    /// Returns the signal behind a published event name, with its payload erased.
    ///
    /// # Why this exists
    ///
    /// Wiring a designer's connection means turning a **name** into a subscription, and the name
    /// is only known at run time. Without this, the host had to write one `match` arm per event per
    /// control — which is exactly the work a designer cannot pre-generate, because it does not know
    /// which wires the user will draw.
    ///
    /// # Contract
    ///
    /// The name must be one the control's capability publishes; a name it does not publish, or an
    /// event whose signal this control does not expose, returns `None`. The default implementation
    /// returns `None` for every name, so a control that has not been converted yet is honest about
    /// it rather than appearing to offer wiring it cannot do.
    ///
    /// The list this resolves against is the one [`connect_event`] validates against, so a name
    /// this returns `Some` for is a name that method accepts — and, with the binder, actually
    /// reaches.
    ///
    /// # Why it is an addition rather than a change
    ///
    /// Every existing accessor (`clicked_signal`, `value_changed`, …) is untouched: they return
    /// concrete types and stay the way a Rust caller should reach a signal. This is the erased
    /// companion for the dynamic path, not a replacement.
    ///
    /// [`connect_event`]: crate::widget::capability::WidgetFactory::connect_event
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        let _ = name;
        None
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
    use crate::compat::{MiniToString, String};
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

    // ── The announced value (BLUE21 P2-8) ──────────────────────────────

    /// A value-bearing control announces where it stands, and a control without a value
    /// concept announces nothing.
    ///
    /// # The defect this pins
    ///
    /// Every control was announced by name only, so assistive technology could say *what* a
    /// slider was but never *where its handle sat* — and a slider, a progress bar and a rating are
    /// exactly the controls whose entire purpose is to communicate a magnitude. The value was
    /// reachable through the property contract all along; nothing carried it to the a11y layer.
    ///
    /// # Why both directions are asserted
    ///
    /// "A slider announces 40" is only half the contract. The other half is that a plain button
    /// announces *no* value: a resolver that fell back to any property it could find would announce
    /// the button's own label as its value, and a screen reader would then read "Open, button,
    /// Open". The negative case is what keeps a label and a value from being conflated.
    #[test]
    fn a_value_bearing_control_announces_its_value_and_a_plain_one_does_not() {
        use crate::widget::display_widgets::progressbar::ProgressBar;
        use crate::widget::display_widgets::slider::Slider;

        let mut slider = Slider::new(Rect::new(0, 0, 200, 24));
        slider.set_range(0, 100);
        slider.set_value(40);
        assert!(
            slider.accessible_value().contains("40"),
            "a slider must announce where its handle sits, got {:?}",
            slider.accessible_value()
        );

        // Moving the control must move what is announced; a value baked in at construction would
        // pass the assertion above and still be useless.
        slider.set_value(75);
        assert!(
            slider.accessible_value().contains("75"),
            "the announcement must track the control, got {:?}",
            slider.accessible_value()
        );

        let mut bar = ProgressBar::new(Rect::new(0, 0, 200, 8));
        bar.set_range(0, 100);
        bar.set_value(25);
        assert!(
            bar.accessible_value().contains("25"),
            "a progress bar announces its progress, got {:?}",
            bar.accessible_value()
        );
        assert!(!bar.accessible_value().is_empty(), "a progress bar announces its progress");

        // A button has a label but no value, and must not announce its label twice.
        let button = Button::new("Open".to_string(), Rect::new(0, 0, 100, 32));
        assert_eq!(button.accessible_value(), "", "a button has no value; its label is its name");
    }

    /// A control that publishes a value announces it through the contract, not a per-kind table.
    ///
    /// The resolver walks [`crate::control_backend::custom::VALUE_PROPERTY_NAMES`], so this checks
    /// the property the walk settles on is one the control genuinely answers — the mechanism that
    /// lets a control added later be announced without editing the trait method.
    #[test]
    fn the_announced_value_comes_from_the_property_contract() {
        use crate::widget::display_widgets::rating::Rating;

        let mut rating = Rating::new(Rect::new(0, 0, 120, 24));
        rating.set_rating(3);

        // The value must be rendered the way a person reads it: a whole number, not `3.0`.
        assert_eq!(rating.accessible_value(), "3", "an integral rating is announced as an integer");

        let properties = rating.properties_dyn().expect("rating publishes a contract");
        let resolved = crate::control_backend::custom::widget_value_property_name(properties);
        assert!(
            resolved.is_some_and(|name| properties.get(name).is_ok()),
            "the resolved name must be one the control actually answers"
        );
    }
}
