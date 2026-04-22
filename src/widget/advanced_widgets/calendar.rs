//! Calendar widget.

use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};

/// Calendar widget.
pub struct Calendar {
    base: BaseWidget,
    selected_date: chrono::NaiveDate,
    minimum_date: chrono::NaiveDate,
    maximum_date: chrono::NaiveDate,
    first_day_of_week: chrono::Weekday,
    grid_visible: bool,
    navigation_bar_visible: bool,
    horizontal_header_visible: bool,
    vertical_header_visible: bool,
    pub selection_changed: Signal1<chrono::NaiveDate>,
}

impl Calendar {
    /// Creates a calendar widget.
    pub fn new(geometry: Rect) -> Self {
        let today = chrono::Local::now().date_naive();

        Self {
            base: BaseWidget::new(WidgetKind::Calendar, geometry, "Calendar"),
            selected_date: today,
            minimum_date: chrono::NaiveDate::from_ymd_opt(1900, 1, 1).unwrap(),
            maximum_date: chrono::NaiveDate::from_ymd_opt(3000, 12, 31).unwrap(),
            first_day_of_week: chrono::Weekday::Mon,
            grid_visible: true,
            navigation_bar_visible: true,
            horizontal_header_visible: true,
            vertical_header_visible: false,
            selection_changed: Signal1::new(),
        }
    }

    /// Returns selected date.
    pub fn selected_date(&self) -> chrono::NaiveDate {
        self.selected_date
    }

    /// Sets selected date.
    pub fn set_selected_date(&mut self, date: chrono::NaiveDate) {
        if self.selected_date != date && date >= self.minimum_date && date <= self.maximum_date {
            self.selected_date = date;
            self.selection_changed.emit(date);
        }
    }

    /// Returns minimum date.
    pub fn minimum_date(&self) -> chrono::NaiveDate {
        self.minimum_date
    }

    /// Sets minimum date.
    pub fn set_minimum_date(&mut self, date: chrono::NaiveDate) {
        self.minimum_date = date;
        if self.selected_date < date {
            self.set_selected_date(date);
        }
    }

    /// Returns maximum date.
    pub fn maximum_date(&self) -> chrono::NaiveDate {
        self.maximum_date
    }

    /// Sets maximum date.
    pub fn set_maximum_date(&mut self, date: chrono::NaiveDate) {
        self.maximum_date = date;
        if self.selected_date > date {
            self.set_selected_date(date);
        }
    }

    /// Returns first day of week.
    pub fn first_day_of_week(&self) -> chrono::Weekday {
        self.first_day_of_week
    }

    /// Sets first day of week.
    pub fn set_first_day_of_week(&mut self, weekday: chrono::Weekday) {
        self.first_day_of_week = weekday;
    }

    /// Returns whether grid is visible.
    pub fn is_grid_visible(&self) -> bool {
        self.grid_visible
    }

    /// Sets grid visibility.
    pub fn set_grid_visible(&mut self, visible: bool) {
        self.grid_visible = visible;
    }

    /// Returns whether navigation bar is visible.
    pub fn is_navigation_bar_visible(&self) -> bool {
        self.navigation_bar_visible
    }

    /// Sets navigation bar visibility.
    pub fn set_navigation_bar_visible(&mut self, visible: bool) {
        self.navigation_bar_visible = visible;
    }

    /// Returns whether horizontal header is visible.
    pub fn is_horizontal_header_visible(&self) -> bool {
        self.horizontal_header_visible
    }

    /// Sets horizontal header visibility.
    pub fn set_horizontal_header_visible(&mut self, visible: bool) {
        self.horizontal_header_visible = visible;
    }

    /// Returns whether vertical header is visible.
    pub fn is_vertical_header_visible(&self) -> bool {
        self.vertical_header_visible
    }

    /// Sets vertical header visibility.
    pub fn set_vertical_header_visible(&mut self, visible: bool) {
        self.vertical_header_visible = visible;
    }

    /// Shows today's date.
    pub fn show_today(&mut self) {
        let today = chrono::Local::now().date_naive();
        self.set_selected_date(today);
    }

    /// Shows next month.
    pub fn show_next_month(&mut self) {
        if let Some(next_month) = self
            .selected_date
            .with_month(self.selected_date.month() + 1)
        {
            self.set_selected_date(next_month);
        } else if let Some(next_year) = self.selected_date.with_year(self.selected_date.year() + 1)
        {
            self.set_selected_date(next_year.with_month(1).unwrap());
        }
    }

    /// Shows previous month.
    pub fn show_previous_month(&mut self) {
        if let Some(prev_month) = self
            .selected_date
            .with_month(self.selected_date.month() - 1)
        {
            self.set_selected_date(prev_month);
        } else if let Some(prev_year) = self.selected_date.with_year(self.selected_date.year() - 1)
        {
            self.set_selected_date(prev_year.with_month(12).unwrap());
        }
    }

    /// Shows next year.
    pub fn show_next_year(&mut self) {
        if let Some(next_year) = self.selected_date.with_year(self.selected_date.year() + 1) {
            self.set_selected_date(next_year);
        }
    }

    /// Shows previous year.
    pub fn show_previous_year(&mut self) {
        if let Some(prev_year) = self.selected_date.with_year(self.selected_date.year() - 1) {
            self.set_selected_date(prev_year);
        }
    }

    /// Returns date at position.
    fn date_at_position(&self, pos: Point) -> Option<chrono::NaiveDate> {
        let rect = self.geometry();
        let cell_width = rect.width / 7.0;
        let cell_height = rect.height / 8.0;

        let col = ((pos.x - rect.x) / cell_width).floor() as i32;
        let row = ((pos.y - rect.y) / cell_height).floor() as i32;

        if col < 0 || col >= 7 || row < 0 || row >= 8 {
            return None;
        }

        // Calculate date based on row and column
        let first_day = self.selected_date.with_day(1).unwrap();
        let first_weekday = first_day.weekday();

        let days_from_start =
            (row - 1) * 7 + col as i32 - (first_weekday.num_days_from_monday() as i32) + 1;

        first_day.checked_add_signed(chrono::Days::new(days_from_start as u64))
    }
}

// Implement Widget trait
impl Widget for Calendar {
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

impl EventHandler for Calendar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);

        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    if let Some(date) = self.date_at_position(*pos) {
                        self.set_selected_date(date);
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    37 => {
                        // Left arrow
                        if let Some(prev_day) = self.selected_date.pred_opt() {
                            self.set_selected_date(prev_day);
                        }
                    }
                    38 => {
                        // Up arrow
                        if let Some(prev_week) =
                            self.selected_date.checked_sub_signed(chrono::Days::new(7))
                        {
                            self.set_selected_date(prev_week);
                        }
                    }
                    39 => {
                        // Right arrow
                        if let Some(next_day) = self.selected_date.succ_opt() {
                            self.set_selected_date(next_day);
                        }
                    }
                    40 => {
                        // Down arrow
                        if let Some(next_week) =
                            self.selected_date.checked_add_signed(chrono::Days::new(7))
                        {
                            self.set_selected_date(next_week);
                        }
                    }
                    33 => {
                        // Page up
                        self.show_previous_month();
                    }
                    34 => {
                        // Page down
                        self.show_next_month();
                    }
                    36 => {
                        // Home
                        self.show_today();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Draw for Calendar {
    fn draw(&self, context: &mut RenderContext) {
        // Draw base widget
        self.base.draw(context);

        let rect = self.geometry();
        let cell_width = rect.width / 7.0;
        let cell_height = rect.height / 8.0;

        // Draw background
        context.fill_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(255, 255, 255),
        );

        // Draw navigation bar if visible
        if self.navigation_bar_visible {
            let nav_height = cell_height;
            context.fill_rect(
                rect.x,
                rect.y,
                rect.width,
                nav_height,
                Color::from_rgb(240, 240, 240),
            );

            // Draw month and year
            let month_year = format!(
                "{} {}",
                self.selected_date.format("%B"),
                self.selected_date.year()
            );
            context.draw_text(
                rect.x + rect.width / 2.0,
                rect.y + nav_height / 2.0,
                &month_year,
                &Font::default(),
                Color::from_rgb(0, 0, 0),
                Alignment::Center,
            );

            // Draw navigation buttons
            let button_size = nav_height * 0.6;
            let button_margin = (nav_height - button_size) / 2.0;

            // Previous month button
            context.draw_line(
                rect.x + button_margin + button_size / 3.0,
                rect.y + nav_height / 2.0,
                rect.x + button_margin + button_size * 2.0 / 3.0,
                rect.y + nav_height / 2.0 - button_size / 3.0,
                Color::from_rgb(100, 100, 100),
            );
            context.draw_line(
                rect.x + button_margin + button_size / 3.0,
                rect.y + nav_height / 2.0,
                rect.x + button_margin + button_size * 2.0 / 3.0,
                rect.y + nav_height / 2.0 + button_size / 3.0,
                Color::from_rgb(100, 100, 100),
            );

            // Next month button
            context.draw_line(
                rect.x + rect.width - button_margin - button_size * 2.0 / 3.0,
                rect.y + nav_height / 2.0 - button_size / 3.0,
                rect.x + rect.width - button_margin - button_size / 3.0,
                rect.y + nav_height / 2.0,
                Color::from_rgb(100, 100, 100),
            );
            context.draw_line(
                rect.x + rect.width - button_margin - button_size * 2.0 / 3.0,
                rect.y + nav_height / 2.0 + button_size / 3.0,
                rect.x + rect.width - button_margin - button_size / 3.0,
                rect.y + nav_height / 2.0,
                Color::from_rgb(100, 100, 100),
            );
        }

        // Draw day headers if visible
        if self.horizontal_header_visible {
            let header_y = rect.y + cell_height;
            let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

            for (i, day) in days.iter().enumerate() {
                let x = rect.x + cell_width * i as f32;
                context.fill_rect(
                    x,
                    header_y,
                    cell_width,
                    cell_height,
                    Color::from_rgb(250, 250, 250),
                );

                if self.grid_visible {
                    context.draw_rect(
                        x,
                        header_y,
                        cell_width,
                        cell_height,
                        Color::from_rgb(230, 230, 230),
                    );
                }

                context.draw_text(
                    x + cell_width / 2.0,
                    header_y + cell_height / 2.0,
                    day,
                    &Font::default(),
                    Color::from_rgb(100, 100, 100),
                    Alignment::Center,
                );
            }
        }

        // Draw calendar grid
        let first_day = self.selected_date.with_day(1).unwrap();
        let first_weekday = first_day.weekday();
        let days_in_month = self.selected_date.day() as i32;

        let start_offset = first_weekday.num_days_from_monday() as i32;

        for row in 0..6 {
            for col in 0..7 {
                let day_index = row * 7 + col - start_offset + 1;
                let x = rect.x + cell_width * col as f32;
                let y = rect.y + cell_height * (row + 2) as f32; // +2 for nav and header rows

                if day_index >= 1 && day_index <= days_in_month {
                    let day_date = first_day.with_day(day_index as u32).unwrap();
                    let is_today = day_date == chrono::Local::now().date_naive();
                    let is_selected = day_date == self.selected_date;

                    // Draw cell background
                    let bg_color = if is_selected {
                        Color::from_rgb(0, 120, 215)
                    } else if is_today {
                        Color::from_rgb(255, 255, 200)
                    } else {
                        Color::from_rgb(255, 255, 255)
                    };

                    context.fill_rect(x, y, cell_width, cell_height, bg_color);

                    // Draw cell border
                    if self.grid_visible {
                        let border_color = if is_selected {
                            Color::from_rgb(0, 100, 200)
                        } else {
                            Color::from_rgb(230, 230, 230)
                        };
                        context.draw_rect(x, y, cell_width, cell_height, border_color);
                    }

                    // Draw day number
                    let text_color = if is_selected {
                        Color::from_rgb(255, 255, 255)
                    } else if day_date.weekday() == chrono::Weekday::Sat
                        || day_date.weekday() == chrono::Weekday::Sun
                    {
                        Color::from_rgb(255, 0, 0)
                    } else {
                        Color::from_rgb(0, 0, 0)
                    };

                    context.draw_text(
                        x + cell_width / 2.0,
                        y + cell_height / 2.0,
                        &format!("{}", day_index),
                        &Font::default(),
                        text_color,
                        Alignment::Center,
                    );
                } else {
                    // Draw empty cell
                    context.fill_rect(
                        x,
                        y,
                        cell_width,
                        cell_height,
                        Color::from_rgb(245, 245, 245),
                    );

                    if self.grid_visible {
                        context.draw_rect(
                            x,
                            y,
                            cell_width,
                            cell_height,
                            Color::from_rgb(230, 230, 230),
                        );
                    }
                }
            }
        }
    }
}
