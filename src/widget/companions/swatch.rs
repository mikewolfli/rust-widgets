//! `Swatch` — a reusable colour grid with selection and activation.

use crate::compat::Vec;
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::{expect_u32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, colors_value, expect_colors, surface_ink};

// ---------------------------------------------------------------------------
// COMP-01 Swatch
// ---------------------------------------------------------------------------

/// Inset around a swatch grid.
const SWATCH_INSET: u32 = 4;
/// Gap between swatch cells.
const SWATCH_GAP: u32 = 4;

/// A reusable colour grid with one selected cell.
///
/// This is the control `ColorDialog`, `ColorPicker` and `ColorHistory` used to each paint by
/// hand. It owns the two facts those copies drifted on: the cell geometry and the meaning of a
/// click (select versus activate).
pub struct Swatch {
    base: BaseWidget,
    colors: Vec<Color>,
    selected_index: usize,
    cell_size: u32,
    columns: usize,
    /// Emitted when a click or arrow key moves the selection, with the new index.
    pub swatch_selected: Signal1<usize>,
    /// Emitted with the colour when a cell is double-clicked or activated from the keyboard.
    pub swatch_activated: Signal1<Color>,
}

impl Swatch {
    /// Creates a swatch grid with the library's default palette.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Swatch, geometry, "Swatch"),
            colors: vec![
                Color::ERROR,
                Color::WARNING,
                Color::SUCCESS,
                Color::PRIMARY,
                Color::rgb(156, 39, 176),
                Color::SECONDARY,
                Color::BLACK,
                Color::WHITE,
            ],
            selected_index: 0,
            cell_size: 24,
            columns: 6,
            swatch_selected: Signal1::new(),
            swatch_activated: Signal1::new(),
        }
    }

    /// Replaces the palette, clamping the selection to the new length.
    pub fn set_colors(&mut self, colors: Vec<Color>) {
        if self.colors == colors {
            return;
        }
        self.colors = colors;
        self.selected_index =
            if self.colors.is_empty() { 0 } else { self.selected_index.min(self.colors.len() - 1) };
        self.base.request_redraw();
    }

    /// Returns the palette.
    pub fn colors(&self) -> &[Color] {
        &self.colors
    }

    /// Selects a cell. An index past the end selects the last cell; an empty grid stays at 0.
    ///
    /// The write is silent: like every other `set_*` accessor it reports state, it does not
    /// pretend the user clicked. User gestures emit [`Self::swatch_selected`].
    pub fn set_selected_index(&mut self, index: usize) {
        let index = if self.colors.is_empty() { 0 } else { index.min(self.colors.len() - 1) };
        if self.selected_index != index {
            self.selected_index = index;
            self.base.request_redraw();
        }
    }

    /// Returns the selected index (`0` for an empty grid).
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Returns the selected colour, when the grid is not empty.
    pub fn selected_color(&self) -> Option<Color> {
        self.colors.get(self.selected_index).copied()
    }

    /// Sets the side of one cell in logical pixels.
    pub fn set_cell_size(&mut self, cell_size: u32) {
        let cell_size = cell_size.max(4);
        if self.cell_size != cell_size {
            self.cell_size = cell_size;
            self.base.request_redraw();
        }
    }

    /// Returns the cell side.
    pub fn cell_size(&self) -> u32 {
        self.cell_size
    }

    /// Sets how many cells a row holds.
    pub fn set_columns(&mut self, columns: usize) {
        let columns = columns.max(1);
        if self.columns != columns {
            self.columns = columns;
            self.base.request_redraw();
        }
    }

    /// Returns the column count.
    pub fn columns(&self) -> usize {
        self.columns
    }

    /// The rectangle of one cell, in window coordinates.
    fn cell_rect(&self, index: usize) -> Rect {
        let columns = self.columns.max(1);
        let row = index / columns;
        let column = index % columns;
        let pitch = self.cell_size.saturating_add(SWATCH_GAP) as i32;
        Rect::new(
            self.geometry().x + SWATCH_INSET as i32 + column as i32 * pitch,
            self.geometry().y + SWATCH_INSET as i32 + row as i32 * pitch,
            self.cell_size,
            self.cell_size,
        )
    }

    /// The cell under `pos`, if any.
    fn index_at(&self, pos: Point) -> Option<usize> {
        (0..self.colors.len()).find(|&index| self.cell_rect(index).contains(pos))
    }

    /// Emits `swatch_activated` for the current cell.
    fn activate_selected(&mut self) {
        if let Some(color) = self.selected_color() {
            self.swatch_activated.emit(color);
        }
    }
}

impl EventHandler for Swatch {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.index_at(*pos) {
                    if self.selected_index != index {
                        self.selected_index = index;
                        self.base.request_redraw();
                        self.swatch_selected.emit(index);
                    }
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some(index) = self.index_at(*pos) {
                    self.set_selected_index(index);
                    self.activate_selected();
                }
            }
            Event::KeyPress { key, .. } => {
                if self.colors.is_empty() {
                    return;
                }
                let columns = self.columns.max(1);
                let last = self.colors.len() - 1;
                let next = match crate::shortcut::Key::from_key_code(*key) {
                    Some(crate::shortcut::Key::Left) => Some(self.selected_index.saturating_sub(1)),
                    Some(crate::shortcut::Key::Right) => Some((self.selected_index + 1).min(last)),
                    Some(crate::shortcut::Key::Up) => {
                        Some(self.selected_index.saturating_sub(columns))
                    }
                    Some(crate::shortcut::Key::Down) => {
                        Some((self.selected_index + columns).min(last))
                    }
                    _ => None,
                };
                if let Some(index) = next {
                    if index != self.selected_index {
                        self.selected_index = index;
                        self.base.request_redraw();
                        self.swatch_selected.emit(index);
                    }
                    return;
                }
                if *key == 13 || *key == 32 {
                    self.activate_selected();
                }
            }
            _ => {}
        }
    }
}

impl Draw for Swatch {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = effective_font(style);

        context.fill_rect(rect, surface);
        context.draw_rect(rect, surface.blend(&ink, 0.18));

        if self.colors.is_empty() {
            context.draw_text_fitted(
                rect,
                "No colors",
                font,
                surface.blend(&ink, 0.55),
                crate::core::HorizontalAlignment::Center,
            );
            return;
        }

        for index in 0..self.colors.len() {
            let cell = self.cell_rect(index);
            if !rect.intersects(&cell) {
                continue;
            }
            context.fill_rect(cell, self.colors[index]);
            context.draw_rect(cell, self.colors[index].contrast_color().with_alpha(90));
            if index == self.selected_index {
                let ring = Rect::new(cell.x - 2, cell.y - 2, cell.width + 4, cell.height + 4);
                context.draw_rect(ring, accent);
            }
        }
    }
}

impl Widget for Swatch {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        let columns = self.columns.max(1) as u32;
        let rows = (self.colors.len().max(1) as u32).div_ceil(columns);
        let pitch = self.cell_size.saturating_add(SWATCH_GAP);
        Size::new(columns * pitch + SWATCH_INSET * 2, rows * pitch + SWATCH_INSET * 2)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "swatch_selected" => {
                Some(EventSignalRef::mapped("swatch_selected", &self.swatch_selected, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            "swatch_activated" => {
                Some(EventSignalRef::mapped("swatch_activated", &self.swatch_activated, |value| {
                    CapabilityValue::String(value.to_hex_rgba())
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for Swatch {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "colors" => Ok(colors_value(&self.colors)),
            "selected_index" => Ok(CapabilityValue::UInt(self.selected_index as u64)),
            "cell_size" => Ok(CapabilityValue::UInt(u64::from(self.cell_size))),
            "columns" => Ok(CapabilityValue::UInt(self.columns as u64)),
            _ => base_property_get(self, name),
        }
    }
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "colors" => {
                self.set_colors(expect_colors(value)?);
                Ok(())
            }
            "selected_index" => {
                self.set_selected_index(expect_usize(value)?);
                Ok(())
            }
            "cell_size" => {
                self.set_cell_size(expect_u32(value)?);
                Ok(())
            }
            "columns" => {
                self.set_columns(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["colors", "selected_index", "cell_size", "columns", BASE_PROPERTY_NAMES]
    }
}
