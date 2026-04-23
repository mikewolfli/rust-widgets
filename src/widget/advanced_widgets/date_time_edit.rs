//! Date-time editor widget.
use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::advanced_widgets::{date_edit::Date, time_edit::Time};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Combined date-time value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
}
impl DateTime {
    pub fn new(date: Date, time: Time) -> Self {
        Self { date, time }
    }
    pub fn is_valid(&self) -> bool {
        self.date.is_valid() && self.time.is_valid()
    }
}
impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.date, self.time)
    }
}
/// Date-time editor widget.
pub struct DateTimeEdit {
    base: BaseWidget,
    datetime: DateTime,
    minimum: DateTime,
    maximum: DateTime,
    display_format: String,
    calendar_popup: bool,
    pub datetime_changed: Signal1<DateTime>,
}
impl DateTimeEdit {
    pub fn new(geometry: Rect) -> Self {
        let min_dt = DateTime::new(Date::new(1752, 9, 14), Time::new(0, 0, 0, 0));
        let max_dt = DateTime::new(Date::new(9999, 12, 31), Time::new(23, 59, 59, 999));
        let now = DateTime::new(Date::today(), Time::new(0, 0, 0, 0));
        Self {
            base: BaseWidget::new(WidgetKind::DateTimePicker, geometry, "DateTimeEdit"),
            datetime: now,
            minimum: min_dt,
            maximum: max_dt,
            display_format: "yyyy-MM-dd HH:mm:ss".to_string(),
            calendar_popup: false,
            datetime_changed: Signal1::new(),
        }
    }
    pub fn datetime(&self) -> DateTime {
        self.datetime
    }
    pub fn date(&self) -> Date {
        self.datetime.date
    }
    pub fn time(&self) -> Time {
        self.datetime.time
    }
    pub fn minimum_datetime(&self) -> DateTime {
        self.minimum
    }
    pub fn maximum_datetime(&self) -> DateTime {
        self.maximum
    }
    pub fn display_format(&self) -> &str {
        &self.display_format
    }
    pub fn calendar_popup(&self) -> bool {
        self.calendar_popup
    }
    pub fn set_datetime(&mut self, dt: DateTime) {
        if dt.is_valid() && dt >= self.minimum && dt <= self.maximum && self.datetime != dt {
            self.datetime = dt;
            self.datetime_changed.emit(dt);
        }
    }
    pub fn set_date(&mut self, date: Date) {
        self.set_datetime(DateTime::new(date, self.datetime.time));
    }
    pub fn set_time(&mut self, time: Time) {
        self.set_datetime(DateTime::new(self.datetime.date, time));
    }
    pub fn set_minimum_datetime(&mut self, dt: DateTime) {
        self.minimum = dt;
    }
    pub fn set_maximum_datetime(&mut self, dt: DateTime) {
        self.maximum = dt;
    }
    pub fn set_display_format(&mut self, fmt: String) {
        self.display_format = fmt;
    }
    pub fn set_calendar_popup(&mut self, popup: bool) {
        self.calendar_popup = popup;
    }
    pub fn step_up(&mut self) {
        let mut t = self.datetime.time;
        t.second += 1;
        if t.second >= 60 {
            t.second = 0;
            t.minute += 1;
        }
        if t.minute >= 60 {
            t.minute = 0;
            if t.hour < 23 {
                t.hour += 1;
            } else {
                t.hour = 0;
                let mut d = self.datetime.date;
                d.day += 1;
                if d.day > d.days_in_month() {
                    d.day = 1;
                    d.month += 1;
                }
                if d.month > 12 {
                    d.month = 1;
                    d.year += 1;
                }
                self.set_datetime(DateTime::new(d, t));
                return;
            }
        }
        self.set_time(t);
    }
    pub fn step_down(&mut self) {
        let mut t = self.datetime.time;
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
                } else {
                    t.hour = 23;
                    let mut d = self.datetime.date;
                    if d.day > 1 {
                        d.day -= 1;
                    } else {
                        if d.month > 1 {
                            d.month -= 1;
                        } else {
                            d.month = 12;
                            d.year -= 1;
                        }
                        d.day = d.days_in_month();
                    }
                    self.set_datetime(DateTime::new(d, t));
                    return;
                }
            }
        }
        self.set_time(t);
    }
}
impl Widget for DateTimeEdit {
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
impl EventHandler for DateTimeEdit {
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
impl Draw for DateTimeEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        context.draw_rect(rect, Color::from_rgb(150, 150, 150));
        let text = self.datetime.to_string();
        context.draw_text(
            Point {
                x: rect.x + 6,
                y: rect.y + (rect.height as i32 / 2),
            },
            &text,
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
    }
}
