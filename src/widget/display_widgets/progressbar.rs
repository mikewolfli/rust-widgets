//! Progress bar widget.

use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};

/// Progress bar widget.
pub struct ProgressBar {
    base: BaseWidget,
    minimum: i32,
    maximum: i32,
    value: i32,
    text_visible: bool,
    orientation: Orientation,
    inverted_appearance: bool,
    pub value_changed: Signal1<i32>,
}

/// Progress bar orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Horizontal progress bar (left to right)
    Horizontal,
    /// Vertical progress bar (bottom to top)
    Vertical,
}

impl Default for Orientation {
    fn default() -> Self {
        Self::Horizontal
    }
}

impl ProgressBar {
    /// Creates a progress bar with default range 0-100.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ProgressBar, geometry, "ProgressBar"),
            minimum: 0,
            maximum: 100,
            value: 0,
            text_visible: true,
            orientation: Orientation::Horizontal,
            inverted_appearance: false,
            value_changed: Signal1::new(),
        }
    }

    /// Returns minimum value.
    pub fn minimum(&self) -> i32 {
        self.minimum
    }

    /// Sets minimum value.
    pub fn set_minimum(&mut self, minimum: i32) {
        self.minimum = minimum;
        if self.maximum < self.minimum {
            self.maximum = self.minimum;
        }
        self.set_value(self.value); // Re-clamp
    }

    /// Returns maximum value.
    pub fn maximum(&self) -> i32 {
        self.maximum
    }

    /// Sets maximum value.
    pub fn set_maximum(&mut self, maximum: i32) {
        self.maximum = maximum;
        if self.minimum > self.maximum {
            self.minimum = self.maximum;
        }
        self.set_value(self.value); // Re-clamp
    }

    /// Sets range.
    pub fn set_range(&mut self, minimum: i32, maximum: i32) {
        self.minimum = minimum;
        self.maximum = maximum.max(minimum);
        self.set_value(self.value); // Re-clamp
    }

    /// Returns current value.
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Sets value, clamped to valid range.
    pub fn set_value(&mut self, value: i32) {
        let clamped = value.clamp(self.minimum, self.maximum);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.value_changed.emit(self.value);
    }

    /// Resets progress bar to minimum value.
    pub fn reset(&mut self) {
        self.set_value(self.minimum);
    }

    /// Returns whether text is visible.
    pub fn is_text_visible(&self) -> bool {
        self.text_visible
    }

    /// Sets text visibility.
    pub fn set_text_visible(&mut self, visible: bool) {
        self.text_visible = visible;
    }

    /// Returns orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Sets orientation.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
    }

    /// Returns whether appearance is inverted.
    pub fn is_inverted_appearance(&self) -> bool {
        self.inverted_appearance
    }

    /// Sets inverted appearance.
    pub fn set_inverted_appearance(&mut self, inverted: bool) {
        self.inverted_appearance = inverted;
    }

    /// Returns progress as percentage (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        if self.maximum == self.minimum {
            return 0.0;
        }
        ((self.value - self.minimum) as f32) / ((self.maximum - self.minimum) as f32)
    }

    /// Returns formatted text for display.
    fn format_text(&self) -> String {
        if !self.text_visible {
            return String::new();
        }

        let percentage = self.progress() * 100.0;
        format!("{}%", percentage.round() as i32)
    }
}

// Implement Widget trait
impl Widget for ProgressBar {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.base.set_min_size(min_size);
    }
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.base.set_max_size(max_size);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base.set_parent(parent);
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.base.set_tooltip(tooltip);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.base.set_style(style);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
    }
}

impl EventHandler for ProgressBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Progress bar is usually non-interactive
    }
}

impl Draw for ProgressBar {
    fn draw(&self, context: &mut RenderContext) {
        // Draw base widget
        self.base.draw(context);

        let rect = self.geometry();
        let progress = self.progress();

        // Draw background
        context.fill_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(240, 240, 240),
        );

        // Draw border
        context.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(200, 200, 200),
        );

        // Draw progress bar
        match self.orientation {
            Orientation::Horizontal => {
                let progress_width = rect.width * progress;
                let x = if self.inverted_appearance {
                    rect.x + rect.width - progress_width
                } else {
                    rect.x
                };
                context.fill_rect(
                    x,
                    rect.y,
                    progress_width,
                    rect.height,
                    Color::from_rgb(0, 120, 215),
                );
            }
            Orientation::Vertical => {
                let progress_height = rect.height * progress;
                let y = if self.inverted_appearance {
                    rect.y
                } else {
                    rect.y + rect.height - progress_height
                };
                context.fill_rect(
                    rect.x,
                    y,
                    rect.width,
                    progress_height,
                    Color::from_rgb(0, 120, 215),
                );
            }
        }

        // Draw text if visible
        let text = self.format_text();
        if !text.is_empty() {
            context.draw_text(
                rect.x + rect.width / 2.0,
                rect.y + rect.height / 2.0,
                &text,
                &Font::default(),
                Color::from_rgb(0, 0, 0),
                Alignment::Center,
            );
        }
    }
}
