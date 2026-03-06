use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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

impl Draw for LCDNumber {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        
        let bg_color = style.background_color.unwrap_or(Color::BLACK);
        let fg_color = style.text_color.unwrap_or(Color::rgb(255, 0, 0));
        
        context.fill_rect(rect, bg_color);
        
        let display_text = self.display_text();
        let digit_width = rect.width / (self.num_digits as f64).max(1.0) as u32;
        let digit_height = rect.height * 7 / 10;
        let segment_width = digit_width / 8;
        let start_x = rect.x + ((rect.width as i32 - digit_width as i32 * display_text.len() as i32) / 2);
        let start_y = rect.y + ((rect.height as i32 - digit_height as i32) / 2);
        
        for (i, ch) in display_text.chars().enumerate() {
            let digit_x = (start_x + i as i32 * digit_width as i32) as u32;
            let digit_y = start_y as u32;
            self.draw_digit(context, ch, digit_x, digit_y, digit_width, digit_height, segment_width, fg_color);
        }
        
        if self.check_overflow() {
            let overflow_color = Color::rgb(255, 255, 0);
            context.fill_circle(
                Point::new(rect.x + 10, rect.y + 10),
                5,
                overflow_color
            );
        }
    }
}

impl LCDNumber {
    fn draw_digit(&self, context: &mut RenderContext, ch: char, x: u32, y: u32, width: u32, height: u32, segment_width: u32, color: Color) {
        let segments = self.get_segments(ch);
        let hw = (segment_width / 2) as i32;
        
        let mid_x = x as i32 + width as i32 / 2;
        let mid_y = y as i32 + height as i32 / 2;
        let top_y = y as i32;
        let bottom_y = y as i32 + height as i32;
        let left_x = x as i32;
        let right_x = x as i32 + width as i32;
        
        if segments[0] {
            self.draw_horizontal_segment(context, left_x + hw, top_y, right_x - hw, top_y + segment_width as i32, color);
        }
        if segments[1] {
            self.draw_vertical_segment(context, right_x - segment_width as i32, top_y + hw, right_x, mid_y - hw, color);
        }
        if segments[2] {
            self.draw_vertical_segment(context, right_x - segment_width as i32, mid_y + hw, right_x, bottom_y - hw, color);
        }
        if segments[3] {
            self.draw_horizontal_segment(context, left_x + hw, bottom_y - segment_width as i32, right_x - hw, bottom_y, color);
        }
        if segments[4] {
            self.draw_vertical_segment(context, left_x, mid_y + hw, left_x + segment_width as i32, bottom_y - hw, color);
        }
        if segments[5] {
            self.draw_vertical_segment(context, left_x, top_y + hw, left_x + segment_width as i32, mid_y - hw, color);
        }
        if segments[6] {
            self.draw_horizontal_segment(context, left_x + hw, mid_y - hw / 2, right_x - hw, mid_y + hw / 2, color);
        }
    }
    
    fn draw_horizontal_segment(&self, context: &mut RenderContext, x1: i32, y1: i32, x2: i32, y2: i32, color: Color) {
        let width = (x2 - x1).max(1) as u32;
        let height = (y2 - y1).max(1) as u32;
        match self.segment_style {
            SegmentStyle::Outline => {
                context.draw_rect_stroke(Rect::new(x1, y1, width, height), color, 1);
            }
            SegmentStyle::Filled => {
                context.fill_rect(Rect::new(x1, y1, width, height), color);
            }
            SegmentStyle::Flat => {
                context.fill_rect(Rect::new(x1, y1, width, height), color);
            }
        }
    }
    
    fn draw_vertical_segment(&self, context: &mut RenderContext, x1: i32, y1: i32, x2: i32, y2: i32, color: Color) {
        let width = (x2 - x1).max(1) as u32;
        let height = (y2 - y1).max(1) as u32;
        match self.segment_style {
            SegmentStyle::Outline => {
                context.draw_rect_stroke(Rect::new(x1, y1, width, height), color, 1);
            }
            SegmentStyle::Filled => {
                context.fill_rect(Rect::new(x1, y1, width, height), color);
            }
            SegmentStyle::Flat => {
                context.fill_rect(Rect::new(x1, y1, width, height), color);
            }
        }
    }
    
    fn get_segments(&self, ch: char) -> [bool; 7] {
        match ch.to_ascii_uppercase() {
            '0' => [true, true, true, true, true, true, false],
            '1' => [false, true, true, false, false, false, false],
            '2' => [true, true, false, true, true, false, true],
            '3' => [true, true, true, true, false, false, true],
            '4' => [false, true, true, false, false, true, true],
            '5' => [true, false, true, true, false, true, true],
            '6' => [true, false, true, true, true, true, true],
            '7' => [true, true, true, false, false, false, false],
            '8' => [true, true, true, true, true, true, true],
            '9' => [true, true, true, true, false, true, true],
            'A' => [true, true, true, false, true, true, true],
            'B' => [false, false, true, true, true, true, true],
            'C' => [true, false, false, true, true, true, false],
            'D' => [false, true, true, true, true, false, true],
            'E' => [true, false, false, true, true, true, true],
            'F' => [true, false, false, false, true, true, true],
            '-' => [false, false, false, false, false, false, true],
            '.' => [false, false, false, false, false, false, false],
            ' ' => [false, false, false, false, false, false, false],
            _ => [false, false, false, false, false, false, false],
        }
    }
}
