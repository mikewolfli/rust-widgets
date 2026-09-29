// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The quote board — a watchlist table of instruments with price, change and volume.
//!
//! # Why it is not a `DataGrid`
//!
//! A grid draws whatever rows it is given and lets each column format itself. A quote
//! board's columns are not arbitrary: the number of decimals follows the instrument, the
//! change column is coloured by sign, and the change *value* is derived from two other
//! columns rather than stored. Encoding those as grid column formatters would mean a
//! `DataGrid` configured to know about previous closes — which is finance knowledge in a
//! generic control.
//!
//! The opposite direction is the real risk: a board built on a grid that then needs a
//! sorting key, a pinned first column and a row-colour rule usually ends up re-deciding
//! what a row is. This one decides once.

use alloc::string::String;
use alloc::vec::Vec;

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::special_widgets::finance::layout::PlotArea;
use crate::widget::special_widgets::finance::types::Quote;
use crate::widget::special_widgets::finance::volume_chart::draw_empty_pane;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The columns a quote board can show.
///
/// A list rather than four booleans, because it is ordered: the order of this vector is
/// the left-to-right order of the columns, which is what a caller actually wants to
/// control and what four flags could not express.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteColumn {
    /// The ticker symbol.
    Symbol,
    /// The display name.
    Name,
    /// The last traded price.
    Last,
    /// The change, in price units. Coloured by sign.
    Change,
    /// The change, as a percentage. Coloured by sign.
    ChangePercent,
    /// The session high.
    High,
    /// The session low.
    Low,
    /// The session volume.
    Volume,
}

impl QuoteColumn {
    /// The column heading.
    pub fn title(self) -> &'static str {
        match self {
            QuoteColumn::Symbol => "Symbol",
            QuoteColumn::Name => "Name",
            QuoteColumn::Last => "Last",
            QuoteColumn::Change => "Change",
            QuoteColumn::ChangePercent => "Chg%",
            QuoteColumn::High => "High",
            QuoteColumn::Low => "Low",
            QuoteColumn::Volume => "Volume",
        }
    }

    /// Whether this column's value is signed and therefore coloured.
    pub fn is_signed(self) -> bool {
        matches!(self, QuoteColumn::Change | QuoteColumn::ChangePercent)
    }

    /// Whether this column is right-aligned, as numbers are.
    pub fn is_numeric(self) -> bool {
        !matches!(self, QuoteColumn::Symbol | QuoteColumn::Name)
    }

    /// The factory spelling of this column.
    pub fn as_str(self) -> &'static str {
        match self {
            QuoteColumn::Symbol => "symbol",
            QuoteColumn::Name => "name",
            QuoteColumn::Last => "last",
            QuoteColumn::Change => "change",
            QuoteColumn::ChangePercent => "change_percent",
            QuoteColumn::High => "high",
            QuoteColumn::Low => "low",
            QuoteColumn::Volume => "volume",
        }
    }

    /// Parses a factory spelling into a column.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "symbol" => QuoteColumn::Symbol,
            "name" => QuoteColumn::Name,
            "last" => QuoteColumn::Last,
            "change" => QuoteColumn::Change,
            "change_percent" => QuoteColumn::ChangePercent,
            "high" => QuoteColumn::High,
            "low" => QuoteColumn::Low,
            "volume" => QuoteColumn::Volume,
            _ => return None,
        })
    }
}

/// Which column the rows are sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuoteSort {
    /// The order the caller supplied, which is often a watchlist's own order.
    #[default]
    None,
    /// By symbol, ascending.
    Symbol,
    /// By last price, descending — the most active at the top.
    LastDescending,
    /// By absolute change, descending — the day's biggest movers first.
    ChangeMagnitude,
}

/// The quote board control.
pub struct QuoteBoard {
    base: BaseWidget,
    quotes: Vec<Quote>,
    /// The stable order the caller supplied, kept separately from the display order so
    /// switching the sort back to `None` restores it rather than inventing an order.
    ///
    /// A snapshot of the quotes rather than a list of indices: any sort permutes
    /// `quotes`, which invalidates every index into it. See `apply_sort`.
    original_order: Vec<Quote>,
    columns: Vec<QuoteColumn>,
    sort: QuoteSort,
    hovered: Option<usize>,
    selected: Option<usize>,
    rising_color: Color,
    falling_color: Color,
    text_color: Color,
    /// Emitted with the symbol when a row is clicked.
    pub quote_clicked: Signal1<String>,
    /// Emitted with the symbol as the pointer moves onto a row.
    pub quote_hovered: Signal1<String>,
    /// Emitted with the symbol when the selected row changes.
    pub selection_changed: Signal1<Option<String>>,
}

impl QuoteBoard {
    /// Creates an empty board.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::QuoteBoard, geometry, "QuoteBoard"),
            quotes: Vec::new(),
            original_order: Vec::new(),
            columns: alloc::vec![
                QuoteColumn::Symbol,
                QuoteColumn::Last,
                QuoteColumn::Change,
                QuoteColumn::ChangePercent,
                QuoteColumn::Volume,
            ],
            sort: QuoteSort::default(),
            hovered: None,
            selected: None,
            rising_color: Color::rgb(38, 166, 91),
            falling_color: Color::rgb(220, 68, 70),
            text_color: Color::rgb(214, 220, 228),
            quote_clicked: Signal1::new(),
            quote_hovered: Signal1::new(),
            selection_changed: Signal1::new(),
        }
    }

    /// The quotes, in display order.
    pub fn quotes(&self) -> &[Quote] {
        &self.quotes
    }

    /// Replaces the quotes and requests a repaint.
    ///
    /// The supplied order is remembered so a sort can be undone. Storing the quotes
    /// themselves — rather than indices into the live vector — is what makes
    /// [`QuoteSort::None`] meaningful: indices would be invalidated by any intervening
    /// sort, and "no sort" would then mean "restore the order that happens to be left
    /// over", which is not the caller's order.
    pub fn set_quotes(&mut self, quotes: Vec<Quote>) {
        self.quotes = quotes;
        self.original_order = self.quotes.clone();
        self.apply_sort();
        self.revalidate_selection();
        self.base.request_redraw();
    }

    /// The visible columns, left to right.
    pub fn columns(&self) -> &[QuoteColumn] {
        &self.columns
    }

    /// Replaces the visible columns.
    ///
    /// An empty list is applied as given: a caller that has hidden every column gets an
    /// empty board rather than a silently restored default, because the default is a
    /// choice the caller may be deliberately making not to have.
    pub fn set_columns(&mut self, columns: Vec<QuoteColumn>) {
        self.columns = columns;
        self.base.request_redraw();
    }

    /// The active sort.
    pub fn sort(&self) -> QuoteSort {
        self.sort
    }

    /// Sets the sort and requests a repaint.
    pub fn set_sort(&mut self, sort: QuoteSort) {
        self.sort = sort;
        self.apply_sort();
        self.revalidate_selection();
        self.base.request_redraw();
    }

    /// The index of the selected row.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    /// The selected quote's symbol.
    pub fn selected_symbol(&self) -> Option<&str> {
        self.selected.and_then(|index| self.quotes.get(index)).map(|quote| quote.symbol.as_str())
    }

    /// Selects a row by index, emitting `selection_changed` when it moves.
    ///
    /// An out-of-range index clears the selection rather than being ignored, so a caller
    /// that has just replaced a shorter quote list and passes its old index gets a
    /// consistent "nothing is selected" rather than a stale highlight.
    pub fn select(&mut self, index: Option<usize>) {
        let next = index.filter(|value| *value < self.quotes.len());
        if next == self.selected {
            return;
        }
        self.selected = next;
        let symbol = self.selected_symbol().map(String::from);
        self.selection_changed.emit(symbol);
        self.base.request_redraw();
    }

    /// The index of the hovered row.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered
    }

    /// The row colours.
    pub fn colors(&self) -> (Color, Color) {
        (self.rising_color, self.falling_color)
    }

    /// Sets the up and down row colours.
    pub fn set_colors(&mut self, rising: Color, falling: Color) {
        self.rising_color = rising;
        self.falling_color = falling;
        self.base.request_redraw();
    }

    /// Reorders the quotes according to the active sort.
    ///
    /// # Why the caller's order is kept as quotes, not as indices
    ///
    /// `QuoteSort::None` promises to restore *the order the caller supplied*, so the
    /// natural implementation is to remember the indices and re-read them. That is what
    /// the first version did, and it was wrong: after `QuoteSort::Symbol` the quotes are
    /// permuted, so a remembered index no longer points at the quote it named. The
    /// restore then produced a third order that was neither the caller's nor the sorted
    /// one — silent, because each step looked correct on its own.
    ///
    /// Keeping a snapshot of the quotes makes the restore independent of whatever any
    /// earlier sort did to the live vector. It costs one clone per quote per
    /// `set_quotes`, which happens on a data update rather than per frame.
    /// `app::tests::the_watchlist_starts_in_the_supplied_order` is the test that found
    /// this; it round-trips through two different sorts before checking.
    fn apply_sort(&mut self) {
        match self.sort {
            QuoteSort::None => {
                if self.original_order.len() == self.quotes.len() {
                    self.quotes.clone_from(&self.original_order);
                }
            }
            QuoteSort::Symbol => {
                self.quotes.sort_by(|left, right| left.symbol.cmp(&right.symbol));
            }
            QuoteSort::LastDescending => {
                self.quotes.sort_by(|left, right| {
                    right.last.partial_cmp(&left.last).unwrap_or(core::cmp::Ordering::Equal)
                });
            }
            QuoteSort::ChangeMagnitude => {
                self.quotes.sort_by(|left, right| {
                    let left_magnitude = left.change().abs();
                    let right_magnitude = right.change().abs();
                    right_magnitude
                        .partial_cmp(&left_magnitude)
                        .unwrap_or(core::cmp::Ordering::Equal)
                });
            }
        }
    }

    /// Drops a selection or hover that no longer points at a row.
    fn revalidate_selection(&mut self) {
        if self.selected.is_some_and(|index| index >= self.quotes.len()) {
            self.selected = None;
        }
        if self.hovered.is_some_and(|index| index >= self.quotes.len()) {
            self.hovered = None;
        }
    }

    /// The height of one row.
    fn row_height(&self) -> i32 {
        // A fixed 22 pixels rather than a share of the pane: a watchlist is scanned
        // vertically and a row height that changed with the pane size would make the same
        // list read differently in two windows.
        22
    }

    /// The y of the first data row, after the header.
    fn first_row_y(&self) -> i32 {
        self.base.geometry().y + self.row_height() + 1
    }

    /// The index of the row at a screen y, accounting for the header.
    /// The row index under a screen y, if any.
    ///
    /// Bounded by the widget's own rectangle on both edges, matching `TreeTable::row_at`.
    /// The upper bound is defensive: with today's row-height formula a pointer below the
    /// board already exceeds `quotes.len()`, so the data bound rejected it. Stating it here
    /// keeps the rejection from depending on that coincidence.
    fn row_at(&self, y: i32) -> Option<usize> {
        let geometry = self.base.geometry();
        if y < geometry.y || y >= geometry.y + geometry.height as i32 {
            return None;
        }
        let offset = y - self.first_row_y();
        if offset < 0 {
            return None;
        }
        let index = (offset / self.row_height()) as usize;
        if index < self.quotes.len() {
            Some(index)
        } else {
            None
        }
    }

    /// The horizontal range of each column, as `(start, width)` pairs.
    ///
    /// Computed once per frame and shared by the header and the rows, which is what keeps
    /// the two aligned. Computing them per row would let a row drift from its heading.
    fn column_ranges(&self) -> Vec<(i32, i32)> {
        let geometry = self.base.geometry();
        let width = geometry.width as i32;
        let count = self.columns.len().max(1) as i32;
        let slot = width / count;
        (0..self.columns.len() as i32)
            .map(|index| {
                let start = geometry.x + index * slot;
                // The last column takes the remaining pixels, so rounding cannot leave a
                // strip of unpainted background at the right edge.
                let column_width =
                    if index == count - 1 { geometry.x + width - start } else { slot };
                (start, column_width)
            })
            .collect()
    }

    /// The text for one cell.
    fn cell_text(&self, quote: &Quote, column: QuoteColumn, decimals: usize) -> String {
        match column {
            QuoteColumn::Symbol => quote.symbol.clone(),
            QuoteColumn::Name => quote.name.clone(),
            QuoteColumn::Last => format_decimal(quote.last, decimals),
            QuoteColumn::Change => {
                let change = quote.change();
                if change.is_finite() {
                    let sign = if change >= 0.0 { "+" } else { "" };
                    alloc::format!("{sign}{}", format_decimal(change, decimals))
                } else {
                    String::from("—")
                }
            }
            QuoteColumn::ChangePercent => {
                let percent = quote.change_percent();
                if percent.is_finite() {
                    let sign = if percent >= 0.0 { "+" } else { "" };
                    alloc::format!("{sign}{:.2}%", percent)
                } else {
                    String::from("—")
                }
            }
            QuoteColumn::High => format_decimal(quote.high, decimals),
            QuoteColumn::Low => format_decimal(quote.low, decimals),
            QuoteColumn::Volume => format_volume(quote.volume),
        }
    }

    /// The decimals to draw prices at, inferred from the quotes.
    fn inferred_decimals(&self) -> usize {
        for quote in &self.quotes {
            if !quote.last.is_finite() {
                continue;
            }
            for decimals in 0..=4usize {
                let scaled = quote.last * 10f64.powi(decimals as i32);
                if (scaled - scaled.round()).abs() < 1e-6 {
                    return decimals;
                }
            }
        }
        2
    }
}

/// Formats a price at a fixed precision.
fn format_decimal(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return String::from("—");
    }
    let factor = 10f64.powi(decimals as i32);
    let scaled = (value * factor).round() as i64;
    let negative = scaled < 0;
    let digits = scaled.unsigned_abs().to_string();
    if decimals == 0 {
        return if negative { alloc::format!("-{digits}") } else { digits };
    }
    let padded = if digits.len() <= decimals {
        let mut padded = String::new();
        for _ in 0..(decimals - digits.len()) {
            padded.push('0');
        }
        padded.push_str(&digits);
        padded
    } else {
        digits
    };
    let split = padded.len() - decimals;
    let sign = if negative { "-" } else { "" };
    alloc::format!("{sign}{}.{}", &padded[..split], &padded[split..])
}

/// Formats a volume with a magnitude suffix.
///
/// A quote board shows volumes in the millions, where the exact digits are unreadable
/// and the magnitude is the whole message. A suffix is what every platform does, and a
/// caller wanting exact figures reads them from `Quote::volume` rather than the cell.
fn format_volume(volume: f64) -> String {
    if !volume.is_finite() {
        return String::from("—");
    }
    if volume.abs() >= 1_000_000_000.0 {
        alloc::format!("{:.2}B", volume / 1_000_000_000.0)
    } else if volume.abs() >= 1_000_000.0 {
        alloc::format!("{:.2}M", volume / 1_000_000.0)
    } else if volume.abs() >= 1_000.0 {
        alloc::format!("{:.2}K", volume / 1_000.0)
    } else {
        alloc::format!("{volume:.0}")
    }
}

impl Widget for QuoteBoard {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
        Some(self)
    }
    impl_widget_property_hooks!();
}

impl crate::event::EventHandler for QuoteBoard {
    fn handle_event(&mut self, event: &crate::event::Event) {
        use crate::event::Event;
        match event {
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                let next = self.row_at(pos.y);
                if next != self.hovered {
                    if let Some(index) = next {
                        if let Some(quote) = self.quotes.get(index) {
                            self.quote_hovered.emit(quote.symbol.clone());
                        }
                    }
                    self.hovered = next;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } if self.hovered.take().is_some() => {
                self.base.request_redraw();
            }
            Event::MousePress { pos, .. } | Event::PointerPress { pos, .. } => {
                if let Some(index) = self.row_at(pos.y) {
                    if let Some(quote) = self.quotes.get(index) {
                        self.quote_clicked.emit(quote.symbol.clone());
                    }
                    self.select(Some(index));
                }
            }
            _ => {}
        }
        self.base.handle_event(event);
    }
}

impl Draw for QuoteBoard {
    fn draw(&mut self, context: &mut RenderContext) {
        let geometry = self.base.geometry();
        if geometry.width == 0 || geometry.height == 0 {
            return;
        }

        // The pane, the header band and the row states are chrome and resolve the explicit style
        // first, then the theme's resolved style for this control, and only then a literal. Every
        // one of them used to be a fixed dark slate, so a light/dark switch left the board's
        // dominant colour unchanged and the rendering census reported it as theme-blind. The
        // rising/falling colours stay the caller's data colours: red and green encode price
        // direction, so remapping them by theme would erase which way the market moved.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("quote_board");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => {
                    (active.colors.background, active.colors.foreground, active.colors.secondary)
                }
                None => (Color::rgb(240, 240, 240), Color::BLACK, Color::rgb(158, 158, 158)),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `quote_board` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and the active theme writes the window fill into `style.background_color`.
        // A pane painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from it,
        // while a colour the caller set still wins.
        let panel = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        context.fill_rect(geometry, panel);
        if self.columns.is_empty() {
            return;
        }

        // Chrome raised from the panel: the header band, the selected/hovered rows and the
        // ordinary column text all read on a light backdrop and on a dark one.
        let header_fill = panel.blend(&ink, 0.10);
        let selected_fill = panel.blend(&foreground, 0.24);
        let hovered_fill = panel.blend(&ink, 0.10);
        let header_text = ink.blend(&secondary, 0.45);
        let body_text = secondary;

        let ranges = self.column_ranges();
        let row_height = self.row_height();
        let decimals = self.inferred_decimals();

        // The header, on a slightly lighter band so it reads as a heading rather than as
        // a first data row.
        context.fill_rect(
            Rect::new(geometry.x, geometry.y, geometry.width, row_height as u32),
            header_fill,
        );
        for (column, (start, width)) in self.columns.iter().zip(ranges.iter()) {
            let font = Font::simple("Sans", HEADING_FONT_SIZE);
            let title = column.title();
            let label_x = heading_origin(*start, *width, title, &font);
            if heading_fits(*width, title, &font) {
                context.draw_text(
                    Point { x: label_x, y: geometry.y + 5 },
                    title,
                    &font,
                    header_text,
                    HorizontalAlignment::Left,
                );
            } else {
                // The column is narrower than its own heading. Left unwrapped, the glyphs
                // would run across the next column's heading, and two headings overlapping
                // is a picture that reads as corruption rather than as truncation. Fit it to
                // the column instead, so the overflow shows as the ellipsis it is.
                let band = Rect::new(
                    *start,
                    geometry.y + 5,
                    (*width).max(1) as u32,
                    font.size().max(1.0) as u32,
                );
                context.draw_text_fitted(
                    band,
                    title,
                    &font,
                    header_text,
                    HorizontalAlignment::Left,
                );
            }
        }

        // The rows, clipped to the pane by the loop bound rather than by a clip push:
        // a board taller than its frame simply does not draw the rows past the end.
        let available = ((geometry.height as i32 - row_height) / row_height).max(0) as usize;

        // BLUE23 §4.6 (BLUE21 D5): a board with columns but no quotes drew its header over an
        // empty body, which reads as a rendering failure rather than as "no data yet". The header
        // is kept — it is what tells a reader what the missing rows would mean — and the body gets
        // the same shared message its finance siblings use.
        if self.quotes.is_empty() {
            let body = Rect::new(
                geometry.x,
                geometry.y + row_height,
                geometry.width,
                geometry.height.saturating_sub(row_height as u32),
            );
            if body.height > 0 {
                draw_empty_pane(
                    context,
                    &PlotArea::of(body),
                    crate::widget::special_widgets::finance::layout::PanelColors {
                        grid: panel.blend(&secondary, 0.45),
                        ..crate::widget::special_widgets::finance::layout::PanelColors::from_parts(
                            panel, ink,
                        )
                    },
                );
            }
            return;
        }

        for (index, quote) in self.quotes.iter().enumerate().take(available) {
            let y = self.first_row_y() + index as i32 * row_height;
            if self.selected == Some(index) {
                context.fill_rect(
                    Rect::new(geometry.x, y, geometry.width, row_height as u32),
                    selected_fill,
                );
            } else if self.hovered == Some(index) {
                context.fill_rect(
                    Rect::new(geometry.x, y, geometry.width, row_height as u32),
                    hovered_fill,
                );
            }

            for (column, (start, width)) in self.columns.iter().zip(ranges.iter()) {
                let start = *start;
                let width = *width;
                let text = self.cell_text(quote, *column, decimals);
                // A signed column is coloured by the quote's direction, not by the text's
                // own sign, so a change and its percentage can never disagree about colour.
                let color = if column.is_signed() {
                    if quote.is_up() {
                        self.rising_color
                    } else {
                        self.falling_color
                    }
                } else if *column == QuoteColumn::Symbol {
                    self.text_color
                } else {
                    body_text
                };
                let font = Font::simple("Sans", BODY_FONT_SIZE);
                // The advance model the renderer draws with, not a per-character guess: a
                // right-aligned number whose reserved width was a guess lands short of its
                // column edge by the difference, and the guess drifts with every glyph that
                // is not one em wide. See `estimate_width`'s removal for the measurement.
                //
                // # Why the cell is fitted rather than placed
                //
                // `cell_text` returns the quote's own `symbol` and `name` unbounded, and
                // `draw_text` places a string without asking whether it fits. A symbol longer
                // than its column therefore painted straight through the next column — the
                // same defect the *heading* had, in the row below it. The heading was fixed by
                // keeping the label inside its column; this is that fix's other half. Fitting
                // is the honest reading: a clipped value reads as truncated, one that has run
                // into its neighbour reads as a corrupted table.
                let text_width = measure_body_text(&text, &font) as i32;
                let cell = if column.is_numeric() {
                    // Reserved from the trailing edge so the digits line up column-wise.
                    Rect::new(start + width - 8 - text_width, y, text_width.max(1) as u32, 0)
                } else {
                    // Reserved from the leading inset to the column's edge.
                    Rect::new(start + 8, y, (width - 16).max(1) as u32, 0)
                };
                let line = context.text_line(Rect { height: ROW_TEXT_HEIGHT, ..cell }, &font);
                context.draw_text_fitted(line, &text, &font, color, HorizontalAlignment::Left);
            }
        }
    }
}

/// The font size the board draws both its headings and its values at.
///
/// One size for the whole board: a heading larger than the column it identifies is what
/// made the first column's label leave the pane, and a heading smaller than the values it
/// titles reads as a caption on the value rather than as its name.
const BODY_FONT_SIZE: f32 = 11.0;

/// The same size, named for the header row so the two call sites state one fact twice.
const HEADING_FONT_SIZE: f32 = BODY_FONT_SIZE;

/// The height of one row's text band, used to derive a value's line box.
///
/// The row pitch (`row_height`) is what the rows are spaced by; a value's *line box* is the
/// font's own, so the two are separate facts and a band equal to the pitch would centre the
/// text on the pitch rather than on the line.
const ROW_TEXT_HEIGHT: u32 = 16;

/// The drawn width of `text` at `font`.
///
/// `RenderContext::draw_text` positions by origin, so this control has to know a
/// right-aligned value's width **before** it calls. The measurement is
/// [`crate::widget::metrics::estimate_text_width`], the crate's single advance model — the
/// same `for_each_cluster` traversal and the same `estimate_cluster_advance` the renderer
/// uses — so measuring here cannot drift from what is painted. The scale is 1.0 because
/// `draw_text` takes the board's rectangle in device pixels already.
fn measure_body_text(text: &str, font: &Font) -> u32 {
    crate::widget::metrics::estimate_text_width(text, font, 1.0)
}

/// The left edge `title` is drawn at, inside the column that occupies `[start, start + width)`.
///
/// # The defect this replaced
///
/// A heading is one string drawn at one origin, so "where it starts" and "how wide it is"
/// describe the same object. The board nevertheless derived the origin *from the width*, as
/// `start + 8 - title_width` — the arithmetic a **right**-aligned label needs — and then drew
/// the glyphs left-aligned from that origin. Every heading was therefore a whole
/// heading-width to the left of its column's inset, and the clamp that was supposed to keep
/// it in the column floored at the **control's** left edge rather than the column's.
///
/// Measured on the census geometry — five columns of 48 px — the old form put `Change` at
/// x 64 inside a column that starts at 96, and `Volume` at 160 inside a column that starts
/// at 192: each heading sat over the *previous* column's values.
///
/// # Why the inset is not always 8 px
///
/// A column can be narrower than its own heading plus the inset on both sides — the census
/// board's 48 px columns hold `"Symbol"`, which measures 40 px. Insisting on the 8 px inset
/// there rejects the margin before the heading and keeps the frame margin instead, pinning
/// the label to `start`. When both cannot fit, the margin that keeps the heading off the
/// **frame** is the one to give up: the pane has a border of its own, and a heading flush
/// with its own column's edge still reads as that column's heading, whereas one flush with
/// the pane's edge reads as the pane's title.
///
/// # Why it is a free function
///
/// So that the placement can be asserted directly. A test that re-states the formula rather
/// than calling it passes with the defect re-injected — which is how the first version of
/// this test was written, and why it was rewritten.
///
/// # Why a heading is always left-aligned
///
/// It is an identifying label, so it belongs at the column's inset whether the column holds
/// numbers or not. Right-aligning it would move the heading every time a value gained a
/// digit.
fn heading_origin(start: i32, width: i32, title: &str, font: &Font) -> i32 {
    let title_width = measure_body_text(title, font) as i32;
    let inset = start + 8;
    let right_most = start + width - 8 - title_width;
    if right_most >= inset {
        // The usual case: the column holds the heading and the inset on both sides.
        inset
    } else {
        // A column too narrow for both. Give up the frame margin, keep the heading inside
        // its own column, and never push it off the column's left edge.
        right_most.max(start)
    }
}

/// Whether `title` fits inside a column `width` px wide with the inset on both sides.
///
/// A caller that paints a heading which does **not** fit should truncate it rather than let
/// it cross into the next column; see `measure_body_text` for the width to fit it to.
fn heading_fits(width: i32, title: &str, font: &Font) -> bool {
    measure_body_text(title, font) as i32 + 16 <= width
}

/// Test module for the quote board.
impl WidgetProperties for QuoteBoard {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "sort" => {
                let token = match self.sort {
                    QuoteSort::None => "none",
                    QuoteSort::Symbol => "symbol",
                    QuoteSort::LastDescending => "last_descending",
                    QuoteSort::ChangeMagnitude => "change_magnitude",
                };
                Ok(CapabilityValue::String(alloc::string::String::from(token)))
            }
            "selected_index" => match self.selected {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "sort" => {
                let CapabilityValue::String(text) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let sort = match text.as_str() {
                    "none" => QuoteSort::None,
                    "symbol" => QuoteSort::Symbol,
                    "last_descending" => QuoteSort::LastDescending,
                    "change_magnitude" => QuoteSort::ChangeMagnitude,
                    _ => return Err(CapabilityAccessError::OutOfRange),
                };
                self.set_sort(sort);
                Ok(())
            }
            "selected_index" => {
                match value {
                    CapabilityValue::UInt(index) => {
                        // An out-of-range index is refused here rather than silently
                        // clearing: the property contract reports what was asked for, and
                        // "select row 99 of 3" is a caller error, not a request to
                        // deselect.
                        if index as usize >= self.quotes.len() {
                            return Err(CapabilityAccessError::OutOfRange);
                        }
                        self.select(Some(index as usize));
                    }
                    CapabilityValue::Null => self.select(None),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "row_height" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["sort", "selected_index", "row_height", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `quote_board` publishes.
    ///
    /// The board renders rows of quotes and has no plotted series to hang an study
    /// on, so `add_overlay` — shared across the finance group's command lists — is
    /// answered as [`CapabilityAccessError::UnsupportedOnWidget`]: the name is real
    /// and this control cannot perform it. That refusal is an honest answer, not a
    /// fix: the `commands` list still advertises an action this control cannot take,
    /// and reconciling it is a registry decision. `set_series` carries the quote set
    /// and is refused as [`CapabilityAccessError::OutOfRange`].
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_overlay" => Err(CapabilityAccessError::UnsupportedOnWidget),
            "set_series" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, EventHandler};
    use crate::widget::special_widgets::finance::types::{fixtures, Quote};
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// The board carries the quotes it was given.
    #[test]
    fn a_board_carries_its_quotes() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        assert!(board.quotes().is_empty());
        board.set_quotes(fixtures::sample_quotes(8));
        assert_eq!(board.quotes().len(), 8);
    }

    /// The default columns are the ones a watchlist is read by.
    #[test]
    fn the_default_columns_are_the_usual_set() {
        let board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        assert_eq!(
            board.columns(),
            &[
                QuoteColumn::Symbol,
                QuoteColumn::Last,
                QuoteColumn::Change,
                QuoteColumn::ChangePercent,
                QuoteColumn::Volume,
            ]
        );
    }

    /// Columns can be replaced, including with nothing.
    #[test]
    fn columns_can_be_replaced() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_columns(alloc::vec![QuoteColumn::Symbol, QuoteColumn::Name]);
        assert_eq!(board.columns().len(), 2);
        board.set_columns(alloc::vec![]);
        assert!(board.columns().is_empty(), "an empty list is honoured as given");
    }

    /// Every column spelling round-trips.
    #[test]
    fn column_names_round_trip() {
        for column in [
            QuoteColumn::Symbol,
            QuoteColumn::Name,
            QuoteColumn::Last,
            QuoteColumn::Change,
            QuoteColumn::ChangePercent,
            QuoteColumn::High,
            QuoteColumn::Low,
            QuoteColumn::Volume,
        ] {
            let name = column.as_str();
            assert_eq!(QuoteColumn::from_name(name), Some(column), "{name} must round-trip");
            assert!(!column.title().is_empty(), "every column needs a heading");
        }
        assert_eq!(QuoteColumn::from_name("nonsense"), None);
    }

    /// The change columns are the signed ones, and the text columns are the non-numeric.
    #[test]
    fn column_roles_are_classified() {
        assert!(QuoteColumn::Change.is_signed());
        assert!(QuoteColumn::ChangePercent.is_signed());
        assert!(!QuoteColumn::Last.is_signed(), "a price is not signed");
        assert!(!QuoteColumn::Symbol.is_numeric());
        assert!(!QuoteColumn::Name.is_numeric());
        assert!(QuoteColumn::Volume.is_numeric());
    }

    /// Sorting by symbol orders the rows.
    #[test]
    fn sorting_by_symbol_orders_the_rows() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(6));
        board.set_sort(QuoteSort::Symbol);
        let symbols: alloc::vec::Vec<&str> =
            board.quotes().iter().map(|quote| quote.symbol.as_str()).collect();
        let mut expected = symbols.clone();
        expected.sort_unstable();
        assert_eq!(symbols, expected);
    }

    /// Sorting by last price is descending.
    #[test]
    fn sorting_by_price_puts_the_highest_first() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(6));
        board.set_sort(QuoteSort::LastDescending);
        let prices: alloc::vec::Vec<f64> = board.quotes().iter().map(|quote| quote.last).collect();
        for window in prices.windows(2) {
            assert!(window[0] >= window[1], "the highest price must come first");
        }
    }

    /// Sorting by change magnitude puts the biggest movers first, either direction.
    #[test]
    fn sorting_by_magnitude_ignores_direction() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(6));
        board.set_sort(QuoteSort::ChangeMagnitude);
        let magnitudes: alloc::vec::Vec<f64> =
            board.quotes().iter().map(|quote| quote.change().abs()).collect();
        for window in magnitudes.windows(2) {
            assert!(window[0] >= window[1], "the biggest movers must come first");
        }
    }

    /// Returning to no sort restores the caller's own order.
    ///
    /// This is why the board stores the original order rather than reversing whatever a
    /// previous sort left behind: a watchlist's order is the caller's, and "unsorted"
    /// has to mean theirs.
    ///
    /// # Why this round-trips through *two* sorts
    ///
    /// The first version checked a single sort, which was not enough to catch the bug it
    /// was written for. The board used to remember the caller's order as **indices** into
    /// the live vector; one sort permutes that vector, so a second sort made the indices
    /// point at the wrong quotes and the restore produced a third order that was neither
    /// the caller's nor the sorted one. With one sort, the permutation happened to be
    /// the identity for this fixture and the test passed.
    ///
    /// Sorting twice with two different keys applies two different permutations, so an
    /// index-based restore cannot coincide with the correct answer by luck.
    #[test]
    fn clearing_the_sort_restores_the_supplied_order() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        // A fixture whose order is deliberately *not* any of the sorts, so each sort
        // below really permutes it. `fixtures::sample_quotes` is already symbol-ordered
        // (`SYM0`, `SYM1`, …), which would make the symbol sort a no-op and weaken this
        // test to one real permutation.
        let mut quotes = fixtures::sample_quotes(6);
        quotes.reverse();
        board.set_quotes(quotes);

        let original: alloc::vec::Vec<alloc::string::String> =
            board.quotes().iter().map(|quote| quote.symbol.clone()).collect();
        let mut by_symbol = original.clone();
        by_symbol.sort();
        assert_ne!(by_symbol, original, "the fixture must not already be symbol-sorted");

        board.set_sort(QuoteSort::LastDescending);
        board.set_sort(QuoteSort::Symbol);
        board.set_sort(QuoteSort::ChangeMagnitude);
        board.set_sort(QuoteSort::None);
        let restored: alloc::vec::Vec<alloc::string::String> =
            board.quotes().iter().map(|quote| quote.symbol.clone()).collect();
        assert_eq!(restored, original, "the caller's order must come back exactly");
    }

    /// Selecting a row emits its symbol, and selecting again does nothing.
    #[test]
    fn selection_emits_and_is_idempotent() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(4));
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        board.selection_changed.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        board.select(Some(1));
        assert_eq!(count.load(Ordering::SeqCst), 1, "a real move emits once");
        assert_eq!(board.selected_symbol(), Some("SYM1"));
        board.select(Some(1));
        assert_eq!(count.load(Ordering::SeqCst), 1, "re-selecting the same row is a no-op");
        board.select(None);
        assert_eq!(count.load(Ordering::SeqCst), 2, "clearing emits");
        assert_eq!(board.selected_symbol(), None);
    }

    /// An out-of-range selection clears rather than pointing at nothing.
    #[test]
    fn an_out_of_range_selection_clears() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(3));
        board.select(Some(99));
        assert_eq!(board.selected_index(), None, "there is no row 99 to select");
    }

    /// An empty board draws without panicking.
    #[test]
    fn an_empty_board_draws_without_panicking() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 480, 240));
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(480, 240), 1.0);
        let mut context = RenderContext::new(&mut backend);
        board.draw(&mut context);
    }

    /// A board with columns but no quotes draws a **`No data` message** over the empty body.
    ///
    /// BLUE23 §4.6 (BLUE21 D5). Before this, the header drew and the body was empty: a reader saw
    /// a labelled grid of nothing, which reads as a rendering failure rather than as "no data
    /// yet".
    ///
    /// # Why the assertion is on the body's y-band, not on the total ink
    ///
    /// The header is deliberately kept (it is what gives the missing rows their meaning), so
    /// "some text was drawn" is already true on the broken version — measured: a mutation that
    /// deletes the message still passed a total-ink assertion. Isolating by y is what makes the
    /// check about the *body*: the message is centred in the region below the header, while the
    /// header's own labels sit inside the first row band.
    #[test]
    fn an_empty_board_draws_the_no_data_message() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 480, 240));
        let empty_svg =
            crate::widget::svg::render_widget_to_svg(&mut board, Rect::new(0, 0, 480, 240));
        // The header occupies row 0; anything at or below the second row is body ink.
        let header_height = board.row_height();
        let body_runs = crate::widget::svg::text_ink_boxes(&empty_svg)
            .into_iter()
            .filter(|(_, top, _, _)| *top >= header_height)
            .count();
        assert!(
            body_runs > 0,
            "the empty body must say so rather than showing a header over nothing: \
             {body_runs} body text runs, header height {header_height}"
        );

        // And a populated board puts *rows* there instead, so the two states are distinguishable.
        let mut filled = QuoteBoard::new(Rect::new(0, 0, 480, 240));
        filled.set_quotes(fixtures::sample_quotes(8));
        let filled_svg =
            crate::widget::svg::render_widget_to_svg(&mut filled, Rect::new(0, 0, 480, 240));
        let filled_runs = crate::widget::svg::text_ink_boxes(&filled_svg)
            .into_iter()
            .filter(|(_, top, _, _)| *top >= header_height)
            .count();
        assert!(
            filled_runs > body_runs,
            "eight quotes must draw more body runs ({filled_runs}) than the message ({body_runs})"
        );
    }

    /// A board with every column selected draws without panicking.
    ///
    /// The column widths are a division of the pane by the column count, so this is the
    /// narrowest-column case.
    #[test]
    fn every_column_draws() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(4));
        board.set_columns(alloc::vec![
            QuoteColumn::Symbol,
            QuoteColumn::Name,
            QuoteColumn::Last,
            QuoteColumn::Change,
            QuoteColumn::ChangePercent,
            QuoteColumn::High,
            QuoteColumn::Low,
            QuoteColumn::Volume,
        ]);
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(640, 240), 1.0);
        let mut context = RenderContext::new(&mut backend);
        board.draw(&mut context);
    }

    /// A heading is placed on the advance model the board draws with, so its origin is the
    /// one the renderer will paint from.
    ///
    /// # The defect this pins
    ///
    /// A heading is one string drawn at one origin, so "where it starts" and "how wide it
    /// is" describe the same object. The board nevertheless computed both, and computed them
    /// from two different facts: the origin came from `estimate_width` (7 px per character)
    /// **subtracted** from `start + 8`, and the glyphs were then painted **left-aligned**
    /// from that origin. The subtraction is what a *right*-aligned label needs. Nothing was
    /// right-aligned, so the subtraction moved every heading a whole heading-width to the
    /// left, and the trailing clamp — which floored at the **control's** left edge rather
    /// than the column's — hid the worst case by pinning `"Symbol"` to x=0, where it ran
    /// into the pane's own frame.
    ///
    /// # Why the assertion names the advance model
    ///
    /// Measured with the board's own `Sans`-11 model, every heading here is within 2 px of
    /// the 7 px-per-character guess (checked below), so replacing the guess with a
    /// measurement on its own moves almost nothing — that is *not* what was wrong, and a
    /// test asserting "the guess is too small" would be pinning a fact that is not a defect.
    /// What was wrong is that the origin was derived as though the label were right-aligned.
    /// The assertion therefore names the *width* the placement must use: the renderer's own
    /// `estimate_text_width`, which is what makes `start + 8` an origin rather than a
    /// right edge. An assertion on the drawn pixel would move with any column-geometry
    /// change; this one fails only if the two facts are re-merged.
    #[test]
    fn a_heading_is_placed_with_the_advance_model_it_is_drawn_with() {
        let font = Font::simple("Sans", BODY_FONT_SIZE);
        for title in ["Symbol", "Name", "Last", "Change", "Chg%", "High", "Low", "Volume"] {
            let measured = measure_body_text(title, &font);
            assert_eq!(
                measured,
                crate::widget::metrics::estimate_text_width(title, &font, 1.0),
                "the heading {title:?} must be measured with the renderer's own advance model"
            );
            assert!(measured > 0, "the heading {title:?} must have a positive width");
        }
        // The 7 px-per-character guess this replaced was **not** the main error: at this font
        // size it happens to be within 2 px of the honest measurement for every heading the
        // board ships. It is still the wrong ruler to keep — it charges an em per cluster, so
        // a narrow glyph and a CJK scalar get the same width — but recording the size of the
        // error here keeps the next reader from mistaking the guess for the root cause and
        // "fixing" only the arithmetic, which would have moved four pixels and left the
        // headings where they were.
        for title in ["Symbol", "Name", "Last", "Change", "Chg%", "High", "Low", "Volume"] {
            let measured = measure_body_text(title, &font) as i32;
            let naive = title.chars().count() as i32 * 7;
            assert!(
                (measured - naive).abs() <= 2,
                "{title:?} measures {measured} against a 7 px-per-character guess of {naive}; \
                 if these diverge the guess is a second, separate defect to fix"
            );
        }
    }

    /// A heading starts at its column's left inset, not a heading-width to the left of it.
    ///
    /// This is the visible half of the defect, and it is asserted at the *narrowest* geometry
    /// the board can be built at — the one in `snapshots/svg/quote_board.svg`, whose columns
    /// are 60 px wide. At a comfortable column width the clamp hides the error by pushing the
    /// heading to the column's left edge, which is only 8 px from the correct answer; the
    /// defect and the clamp are 8 px apart there and a passing assertion proves little. At
    /// 60 px the same defect is visible as "the heading ran into the pane's frame".
    ///
    /// # Why this calls `heading_origin` rather than re-stating the formula
    ///
    /// The first version of this test re-stated `(start + 8).min(..).max(..)` inline. It
    /// passed with the defect re-injected into `draw`, because it was asserting against its
    /// own copy of the formula and never touched the shipped one. Calling the function the
    /// control calls is what makes the assertion load-bearing.
    #[test]
    fn a_heading_starts_at_its_columns_inset() {
        let font = Font::simple("Sans", BODY_FONT_SIZE);
        for title in ["Symbol", "Name", "Last", "Change", "Chg%", "High", "Low", "Volume"] {
            let title_width = measure_body_text(title, &font) as i32;
            // A column is `width` px starting at `start`.
            for (start, width) in [(0, 60), (60, 60), (120, 60), (180, 60), (0, 48), (0, 80)] {
                let label_x = heading_origin(start, width, title, &font);
                assert!(
                    label_x >= start,
                    "the heading {title:?} starts at {label_x}, left of its column at {start}"
                );
                assert!(
                    label_x + title_width <= start + width,
                    "the heading {title:?} ends at {}, past its column's right edge {} \
                     (it starts at {label_x} and measures {title_width})",
                    label_x + title_width,
                    start + width
                );
                // The inset is the point: a heading that fits sits on it. The old form could
                // not reach it, because it subtracted the heading's width from the inset.
                if title_width + 16 <= width {
                    assert_eq!(
                        label_x,
                        start + 8,
                        "the heading {title:?} fits {width} px and must start at its inset"
                    );
                }
            }
        }
    }

    /// The shipped placement keeps `"Symbol"` inside its own 48 px column.
    ///
    /// The concrete case from `snapshots/svg/quote_board.svg`, whose board is the census
    /// rectangle and ships the **five** default columns — so every column is 48 px and the
    /// first one holds `"Symbol"`, which measures 40 px. The heading cannot have an 8 px
    /// inset there and still fit; the assertion is that it nevertheless stays in its own
    /// column rather than being pushed to the pane's edge, and never crosses into the next
    /// column, which is what the old form did for `Change` and `Volume`.
    #[test]
    fn the_first_heading_stays_inside_a_narrow_first_column() {
        let font = Font::simple("Sans", BODY_FONT_SIZE);
        let title = QuoteColumn::Symbol.title();
        let label_x = heading_origin(0, 48, title, &font);
        let title_width = measure_body_text(title, &font) as i32;
        assert!(label_x >= 0, "'Symbol' must not start left of its column, got {label_x}");
        assert!(
            title_width + 2 * 8 > 48,
            "this case is only interesting while the heading cannot hold both insets: \
             '{title}' measures {title_width} in a 48 px column"
        );
        assert!(
            label_x + title_width <= 48,
            "'Symbol' ends at {} in a 48 px column (starts at {label_x}, measures {title_width})",
            label_x + title_width
        );
    }

    /// No heading is ever drawn outside the column it names.
    ///
    /// This is the property that the old form broke on the census geometry: `Change` and
    /// `Volume` were placed over the column to their left, because their origin was derived
    /// from a width that a previous, narrower heading had contributed to.
    ///
    /// The width asserted against is the width the board will **paint**, which is why this
    /// goes through `heading_fits` rather than assuming the nominal measurement: a column too
    /// narrow for its heading at all is truncated to an ellipsis, and asserting the nominal
    /// width there would be asserting a number the board never draws.
    #[test]
    fn no_heading_escapes_its_own_column() {
        let font = Font::simple("Sans", BODY_FONT_SIZE);
        // The census board: 240 px, five default columns, so 48 px each — plus widths a host
        // could set, including two no heading can fit into.
        for (start, width) in [(0, 48), (48, 48), (96, 48), (144, 48), (192, 48), (0, 30), (0, 200)]
        {
            for column in [
                QuoteColumn::Symbol,
                QuoteColumn::Name,
                QuoteColumn::Last,
                QuoteColumn::Change,
                QuoteColumn::ChangePercent,
                QuoteColumn::High,
                QuoteColumn::Low,
                QuoteColumn::Volume,
            ] {
                let title = column.title();
                let label_x = heading_origin(start, width, title, &font);
                assert!(
                    label_x >= start,
                    "{title:?} starts at {label_x}, left of its column at {start}"
                );
                // What the board paints: the measured heading when it fits, otherwise the
                // fitted (truncated) string, which is never wider than the column.
                let painted = if heading_fits(width, title, &font) {
                    measure_body_text(title, &font) as i32
                } else {
                    // `draw_text_fitted` fits inside `width` less one inset at each end.
                    (width - 16).max(0)
                };
                assert!(
                    label_x + painted <= start + width,
                    "{title:?} ends at {} past its column's right edge {} (starts at {label_x}, \
                     paints {painted} px of a {width} px column)",
                    label_x + painted,
                    start + width
                );
            }
        }
    }

    /// A malformed quote draws without panicking.
    #[test]
    fn a_malformed_quote_draws_without_panicking() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 480, 240));
        let mut quote = Quote::new("BAD", f64::NAN, 0.0);
        quote.high = f64::INFINITY;
        quote.low = f64::NEG_INFINITY;
        quote.volume = f64::NAN;
        board.set_quotes(alloc::vec![quote]);
        let mut backend =
            crate::render::SoftwarePaintBackend::new(crate::core::Size::new(480, 240), 1.0);
        let mut context = RenderContext::new(&mut backend);
        board.draw(&mut context);
    }

    /// Colours are stored and returned.
    #[test]
    fn colours_round_trip() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 480, 240));
        let (up, down) = board.colors();
        assert_ne!(up, down);
        board.set_colors(crate::core::Color::rgb(1, 2, 3), crate::core::Color::rgb(4, 5, 6));
        assert_eq!(
            board.colors(),
            (crate::core::Color::rgb(1, 2, 3), crate::core::Color::rgb(4, 5, 6))
        );
    }

    /// Clicking a row emits its symbol and selects it.
    #[test]
    fn clicking_a_row_emits_and_selects() {
        let mut board = QuoteBoard::new(Rect::new(0, 0, 640, 240));
        board.set_quotes(fixtures::sample_quotes(5));
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        board.quote_clicked.connect(move |_| {
            sink.fetch_add(1, Ordering::SeqCst);
        });
        board.handle_event(&crate::event::Event::MousePress {
            pos: crate::core::Point { x: 100, y: 40 },
            button: 0,
            modifiers: 0
        });
        assert_eq!(count.load(Ordering::SeqCst), 1, "a press on a row emits its symbol");
        assert!(board.selected_index().is_some(), "and selects it");
    }
    /// Hover never resolves outside the board's own rectangle.
    ///
    /// The sibling `TreeTable::row_at` checks both edges; this one checked neither the top nor
    /// the bottom. Today the data bound makes a below-the-board pointer resolve to `None`
    /// anyway, so this pins the property rather than a live misbehaviour.
    #[test]
    fn quote_board_hover_is_bounded_by_the_geometry() {
        let geometry = Rect::new(0, 20, 400, 120);
        let hover_at = |y: i32| {
            let mut board = QuoteBoard::new(geometry);
            board.set_quotes(fixtures::sample_quotes(3));
            board.handle_event(&Event::MouseMove { pos: Point::new(150, y) });
            board.hovered_index()
        };

        assert_eq!(hover_at(19), None, "above the widget");
        assert_eq!(hover_at(140), None, "the bottom edge is outside");
        assert_eq!(hover_at(500), None, "far below the widget");
    }
}
