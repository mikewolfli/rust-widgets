//! Common widget contract implemented by all widget models.

use super::{BaseWidget, WidgetKind};
use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::EventHandler;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use std::any::Any;

/// Common widget contract implemented by all widget models.
pub trait Widget: EventHandler + Any {
    /// Returns shared base widget state for default trait delegation.
    fn base(&self) -> &BaseWidget {
        panic!(
            "Widget::base() not implemented — override in {}",
            std::any::type_name::<Self>()
        );
    }
    /// Returns mutable base widget state for default trait delegation.
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
    fn geometry(&self) -> Rect {
        self.base().geometry()
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base_mut().set_geometry(geometry);
    }
    /// Returns widget rectangle aliasing `geometry()`.
    fn rect(&self) -> Rect {
        self.geometry()
    }
    /// Sets widget rectangle aliasing `set_geometry()`.
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
    fn parent(&self) -> Option<ObjectId> {
        self.base().parent()
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base_mut().set_parent(parent);
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base_mut().add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base_mut().remove_child(child);
    }
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
    fn is_visible(&self) -> bool {
        self.base().is_visible()
    }
    fn set_visible(&mut self, visible: bool) {
        if visible {
            self.show();
        } else {
            self.hide();
        }
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base_mut().set_enabled(enabled);
    }
    fn is_enabled(&self) -> bool {
        self.base().is_enabled()
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.base_mut().set_tooltip(tooltip);
    }
    fn tooltip(&self) -> &str {
        self.base().tooltip()
    }
    fn dpi_scale(&self) -> f32 {
        self.base().dpi_scale()
    }
    fn set_dpi_scale(&mut self, scale: f32) {
        self.base_mut().set_dpi_scale(scale);
    }
    fn set_translated_tooltip(&mut self, key: &str) {
        self.base_mut().set_translated_tooltip(key);
    }
    fn style(&self) -> &WidgetStyle {
        self.base().style()
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.base_mut().set_style(style);
    }
    /// Returns optional background color shorthand.
    fn background_color(&self) -> Option<Color> {
        self.style().background_color
    }
    /// Sets optional background color shorthand.
    fn set_background_color(&mut self, color: Option<Color>) {
        let mut style = self.style().clone();
        style.background_color = color;
        self.set_style(style);
    }
    /// Returns optional foreground (text) color shorthand.
    fn foreground_color(&self) -> Option<Color> {
        self.style().text_color
    }
    /// Sets optional foreground (text) color shorthand.
    fn set_foreground_color(&mut self, color: Option<Color>) {
        let mut style = self.style().clone();
        style.text_color = color;
        self.set_style(style);
    }
    /// Returns optional font shorthand.
    fn font(&self) -> Option<&Font> {
        self.style().font.as_ref()
    }
    /// Sets optional font shorthand.
    fn set_font(&mut self, font: Option<Font>) {
        let mut style = self.style().clone();
        style.font = font;
        self.set_style(style);
    }
    /// Returns optional border color shorthand.
    fn border_color(&self) -> Option<Color> {
        self.style().border_color
    }
    /// Returns border width shorthand.
    fn border_width(&self) -> u32 {
        self.style().border_width
    }
    /// Returns border radius shorthand.
    fn border_radius(&self) -> u32 {
        self.style().border_radius
    }
    /// Sets optional border color shorthand.
    fn set_border_color(&mut self, color: Option<Color>) {
        let mut style = self.style().clone();
        style.border_color = color;
        self.set_style(style);
    }
    /// Sets border width shorthand.
    fn set_border_width(&mut self, width: u32) {
        let mut style = self.style().clone();
        style.border_width = width;
        self.set_style(style);
    }
    /// Sets border radius shorthand.
    fn set_border_radius(&mut self, radius: u32) {
        let mut style = self.style().clone();
        style.border_radius = radius;
        self.set_style(style);
    }
    /// Sets border shorthand in one call.
    fn set_border(&mut self, color: Option<Color>, width: u32, radius: u32) {
        let mut style = self.style().clone();
        style.border_color = color;
        style.border_width = width;
        style.border_radius = radius;
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
        let mut style = self.style().clone();
        style.padding = padding;
        self.set_style(style);
    }
    /// Updates widget margin while preserving other style properties.
    fn set_margin(&mut self, margin: Margin) {
        let mut style = self.style().clone();
        style.margin = margin;
        self.set_style(style);
    }
    /// Returns connection scope used to auto-disconnect slots when widget drops.
    fn connection_scope(&self) -> &ConnectionScope {
        self.base().connection_scope()
    }
    /// Optional clicked signal (legacy API compatibility).
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
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base().hover_signal()
    }
    /// Emits on mouse/pointer press interactions.
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base().mouse_down_signal()
    }
    /// Emits on mouse/pointer release interactions.
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base().mouse_up_signal()
    }
    /// Emits on keyboard press interactions.
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base().key_down_signal()
    }
    /// Emits on keyboard release interactions.
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base().key_up_signal()
    }
    /// Emits when logical focus is gained.
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base().focus_gained_signal()
    }
    /// Emits when logical focus is lost.
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base().focus_lost_signal()
    }
    /// Emits when redraw is requested.
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base().redraw_requested_signal()
    }
    /// Emits when layout pass is requested.
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base().layout_requested_signal()
    }
    /// Requests redraw and emits redraw signal.
    fn request_redraw(&self) {
        self.redraw_requested_signal().emit();
    }
    /// Requests layout and emits layout signal.
    fn request_layout(&self) {
        self.layout_requested_signal().emit();
    }
    /// Returns the preferred size hint for layout calculations.
    fn size_hint(&self) -> Size {
        self.size()
    }

    /// Checks whether the given point falls within this widget's interactive area.
    ///
    /// By default, this respects the touch-target expansion set via
    /// `WidgetStyle::with_touch_target()`, so that small widgets remain
    /// easily tappable on touch devices.
    fn contains_point(&self, point: Point) -> bool {
        self.base().contains_point_with_touch_expansion(point)
    }
}
