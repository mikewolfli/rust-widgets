//! Time editor widget.

use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Time value (hour, minute, second, millisecond).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Time {
    pub hour: u8,   // 0-23
    pub minute: u8, // 0-59
    pub second: u8, // 0-59
    pub msec: u16,  // 0-999
}

impl Time {
    pub fn new(hour: u8, minute: u8, second: u8, msec: u16) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
            second: second.min(59),
            msec: msec.min(999),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.hour <= 23 && self.minute <= 59 && self.second <= 59 && self.msec <= 999
    }

    pub fn to_msecs_since_midnight(&self) -> u32 {
        (self.hour as u32 * 3600 + self.minute as u32 * 60 + self.second as u32) * 1000
            + self.msec as u32
    }
}

impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}

/// Time editor widget.
pub struct TimeEdit {
    base: BaseWidget,
    time: Time,
    minimum: Time,
    maximum: Time,
    display_format: String,
    pub time_changed: Signal1<Time>,
}

impl TimeEdit {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TimeEdit, geometry, "TimeEdit"),
            time: Time::new(0, 0, 0, 0),
            minimum: Time::new(0, 0, 0, 0),
            maximum: Time::new(23, 59, 59, 999),
            display_format: "HH:mm:ss".to_string(),
            time_changed: Signal1::new(),
        }
    }

    pub fn time(&self) -> Time {
        self.time
    }
    pub fn minimum_time(&self) -> Time {
        self.minimum
    }
    pub fn maximum_time(&self) -> Time {
        self.maximum
    }
    pub fn display_format(&self) -> &str {
        &self.display_format
    }

    pub fn set_time(&mut self, time: Time) {
        if time.is_valid() && time >= self.minimum && time <= self.maximum && self.time != time {
            self.time = time;
            self.time_changed.emit(time);
        }
    }

    pub fn set_minimum_time(&mut self, time: Time) {
        self.minimum = time;
    }
    pub fn set_maximum_time(&mut self, time: Time) {
        self.maximum = time;
    }
    pub fn set_time_range(&mut self, min: Time, max: Time) {
        self.minimum = min;
        self.maximum = max;
    }
    pub fn set_display_format(&mut self, fmt: String) {
        self.display_format = fmt;
    }

    pub fn step_up(&mut self) {
        let mut t = self.time;
        t.second += 1;
        if t.second >= 60 {
            t.second = 0;
            t.minute += 1;
        }
        if t.minute >= 60 {
            t.minute = 0;
            if t.hour < 23 {
                t.hour += 1;
            }
        }
        self.set_time(t);
    }

    pub fn step_down(&mut self) {
        let mut t = self.time;
        if t.second > 0 {
            t.second -= 1;
        } else {
            t.second = 59;
            if t.minute > 0 {
                t.minute -= 1;
            } else {
                t.minute = 59;
                if t.hour > 0 {
                    t.hour -= 1;
                }
            }
        }
        self.set_time(t);
    }
}

impl Widget for TimeEdit {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, g: Rect) {
        self.base.set_geometry(g);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, s: Option<Size>) {
        self.base.set_min_size(s);
    }
    fn set_max_size(&mut self, s: Option<Size>) {
        self.base.set_max_size(s);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, p: Option<ObjectId>) {
        self.base.set_parent(p);
    }
    fn add_child(&mut self, c: ObjectId) {
        self.base.add_child(c);
    }
    fn remove_child(&mut self, c: ObjectId) {
        self.base.remove_child(c);
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
    fn set_enabled(&mut self, e: bool) {
        self.base.set_enabled(e);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, t: String) {
        self.base.set_tooltip(t);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, s: WidgetStyle) {
        self.base.set_style(s);
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

impl EventHandler for TimeEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, .. } => match *key {
                38 => self.step_up(),
                40 => self.step_down(),
                _ => {}
            },
            _ => {}
        }
    }
}

impl Draw for TimeEdit {
    fn draw(&self, context: &mut RenderContext) {
        self.base.draw(context);
        let rect = self.geometry();
        let spin_width = 16.0;

        context.fill_rect(
            rect.x,
            rect.y,
            rect.width - spin_width,
            rect.height,
            Color::from_rgb(255, 255, 255),
        );
        context.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(150, 150, 150),
        );
        context.draw_text(
            rect.x + 4.0,
            rect.y + rect.height / 2.0,
            &self.time.to_string(),
            &Font::default(),
            Color::from_rgb(0, 0, 0),
            Alignment::Left,
        );

        let btn_x = rect.x + rect.width - spin_width;
        let btn_h = rect.height / 2.0;
        context.fill_rect(
            btn_x,
            rect.y,
            spin_width,
            btn_h,
            Color::from_rgb(240, 240, 240),
        );
        context.fill_rect(
            btn_x,
            rect.y + btn_h,
            spin_width,
            btn_h,
            Color::from_rgb(240, 240, 240),
        );
        context.draw_line(
            btn_x,
            rect.y,
            btn_x,
            rect.y + rect.height,
            Color::from_rgb(150, 150, 150),
        );
        context.draw_line(
            btn_x,
            rect.y + btn_h,
            rect.x + rect.width,
            rect.y + btn_h,
            Color::from_rgb(150, 150, 150),
        );
        let mid_x = btn_x + spin_width / 2.0;
        context.draw_text(
            mid_x,
            rect.y + btn_h / 2.0,
            "▲",
            &Font::default(),
            Color::from_rgb(80, 80, 80),
            Alignment::Center,
        );
        context.draw_text(
            mid_x,
            rect.y + btn_h + btn_h / 2.0,
            "▼",
            &Font::default(),
            Color::from_rgb(80, 80, 80),
            Alignment::Center,
        );
    }
}
