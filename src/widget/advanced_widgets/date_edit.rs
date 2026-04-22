//! Date editor widget.

use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Date value (year, month, day).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u8, // 1-12
    pub day: u8,   // 1-31
}

impl Date {
    pub fn new(year: i32, month: u8, day: u8) -> Self {
        Self {
            year,
            month: month.clamp(1, 12),
            day: day.clamp(1, 31),
        }
    }

    pub fn today() -> Self {
        Self {
            year: 2024,
            month: 1,
            day: 1,
        }
    }

    pub fn days_in_month(&self) -> u8 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if self.is_leap_year() {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }

    pub fn is_leap_year(&self) -> bool {
        (self.year % 4 == 0 && self.year % 100 != 0) || (self.year % 400 == 0)
    }

    pub fn is_valid(&self) -> bool {
        self.month >= 1 && self.month <= 12 && self.day >= 1 && self.day <= self.days_in_month()
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Date editor widget.
pub struct DateEdit {
    base: BaseWidget,
    date: Date,
    minimum: Date,
    maximum: Date,
    display_format: String,
    calendar_popup: bool,
    pub date_changed: Signal1<Date>,
}

impl DateEdit {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DateEdit, geometry, "DateEdit"),
            date: Date::today(),
            minimum: Date::new(1752, 9, 14),
            maximum: Date::new(9999, 12, 31),
            display_format: "yyyy-MM-dd".to_string(),
            calendar_popup: false,
            date_changed: Signal1::new(),
        }
    }

    pub fn date(&self) -> Date {
        self.date
    }
    pub fn minimum_date(&self) -> Date {
        self.minimum
    }
    pub fn maximum_date(&self) -> Date {
        self.maximum
    }
    pub fn display_format(&self) -> &str {
        &self.display_format
    }
    pub fn calendar_popup(&self) -> bool {
        self.calendar_popup
    }

    pub fn set_date(&mut self, date: Date) {
        if date.is_valid() && date >= self.minimum && date <= self.maximum {
            if self.date != date {
                self.date = date;
                self.date_changed.emit(date);
            }
        }
    }

    pub fn set_minimum_date(&mut self, date: Date) {
        self.minimum = date;
    }
    pub fn set_maximum_date(&mut self, date: Date) {
        self.maximum = date;
    }
    pub fn set_date_range(&mut self, min: Date, max: Date) {
        self.minimum = min;
        self.maximum = max;
    }
    pub fn set_display_format(&mut self, fmt: String) {
        self.display_format = fmt;
    }
    pub fn set_calendar_popup(&mut self, popup: bool) {
        self.calendar_popup = popup;
    }

    pub fn step_up(&mut self) {
        let mut d = self.date;
        d.day += 1;
        if d.day > d.days_in_month() {
            d.day = 1;
            d.month += 1;
        }
        if d.month > 12 {
            d.month = 1;
            d.year += 1;
        }
        self.set_date(d);
    }

    pub fn step_down(&mut self) {
        let mut d = self.date;
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
        self.set_date(d);
    }
}

impl Widget for DateEdit {
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

impl EventHandler for DateEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, .. } => match *key {
                38 => self.step_up(),   // Up arrow
                40 => self.step_down(), // Down arrow
                _ => {}
            },
            _ => {}
        }
    }
}

impl Draw for DateEdit {
    fn draw(&self, context: &mut RenderContext) {
        self.base.draw(context);
        let rect = self.geometry();
        let spin_width = 16.0;

        // Background
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

        // Date text
        context.draw_text(
            rect.x + 4.0,
            rect.y + rect.height / 2.0,
            &self.date.to_string(),
            &Font::default(),
            Color::from_rgb(0, 0, 0),
            Alignment::Left,
        );

        // Spin buttons
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

        // Up/Down arrows
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
