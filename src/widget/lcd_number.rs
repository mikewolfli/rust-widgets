use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Widget, WidgetKind};

/// LCD number display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LCDNumberMode {
    /// Display hexadecimal numbers.
    Hex,
    /// Display decimal numbers.
    Dec,
    /// Display octal numbers.
    Oct,
    /// Display binary numbers.
    Bin,
}

/// LCD segment style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentStyle {
    /// Outline style.
    Outline,
    /// Filled style.
    Filled,
    /// Flat style.
    Flat,
}

/// LCD number widget.
pub struct LCDNumber {
    base: BaseWidget,
    value: f64,
    min_value: f64,
    max_value: f64,
    num_digits: i32,
    small_decimal_point: bool,
    mode: LCDNumberMode,
    segment_style: SegmentStyle,
    /// Emitted when the value changes.
    pub value_changed: Signal1<f64>,
    /// Emitted when the display is overflowed.
    pub overflow: GenericSignal,
}

impl LCDNumber {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LCDNumber, geometry, "LCDNumber"),
            value: 0.0,
            min_value: -999999.0,
            max_value: 999999.0,
            num_digits: 6,
            small_decimal_point: false,
            mode: LCDNumberMode::Dec,
            segment_style: SegmentStyle::Filled,
            value_changed: Signal1::new(),
            overflow: GenericSignal::new(),
        }
    }

    pub fn value(&self) -> f64 {
        self.value
    }
    pub fn min_value(&self) -> f64 {
        self.min_value
    }
    pub fn max_value(&self) -> f64 {
        self.max_value
    }
    pub fn num_digits(&self) -> i32 {
        self.num_digits
    }
    pub fn is_small_decimal_point(&self) -> bool {
        self.small_decimal_point
    }
    pub fn mode(&self) -> LCDNumberMode {
        self.mode
    }
    pub fn segment_style(&self) -> SegmentStyle {
        self.segment_style
    }

    pub fn set_value(&mut self, value: f64) {
        let clamped = value.clamp(self.min_value, self.max_value);
        if self.value != clamped {
            self.value = clamped;
            self.value_changed.emit(clamped);
            self.base.request_redraw();
        }
    }
    pub fn set_min_value(&mut self, min: f64) {
        self.min_value = min;
        self.set_value(self.value);
    }
    pub fn set_max_value(&mut self, max: f64) {
        self.max_value = max;
        self.set_value(self.value);
    }
    pub fn set_num_digits(&mut self, digits: i32) {
        self.num_digits = digits.max(1);
        self.base.request_redraw();
    }
    pub fn set_small_decimal_point(&mut self, small: bool) {
        self.small_decimal_point = small;
        self.base.request_redraw();
    }
    pub fn set_mode(&mut self, mode: LCDNumberMode) {
        self.mode = mode;
        self.base.request_redraw();
    }
    pub fn set_segment_style(&mut self, style: SegmentStyle) {
        self.segment_style = style;
        self.base.request_redraw();
    }

    pub fn check_overflow(&self) -> bool {
        self.value < self.min_value || self.value > self.max_value
    }

    pub fn display_text(&self) -> String {
        match self.mode {
            LCDNumberMode::Hex => format!("{:X}", self.value as i64),
            LCDNumberMode::Dec => format!("{}", self.value),
            LCDNumberMode::Oct => format!("{:o}", self.value as i64),
            LCDNumberMode::Bin => format!("{:b}", self.value as i64),
        }
    }
}

impl Widget for LCDNumber {
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
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
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

impl EventHandler for LCDNumber {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}
