// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The document-outline side panel for `MarkdownEditor`.

use super::*;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_string_list, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Row height of one outline entry.
pub(crate) const OUTLINE_ROW_HEIGHT: i32 = 20;
/// How far each heading level indents its title.
const OUTLINE_INDENT: i32 = 12;

/// A document-outline side panel for a [`MarkdownEditor`].
///
/// The editor already computes its headings ([`MarkdownEditor::outline`]) and can navigate to
/// one ([`MarkdownEditor::goto_outline`]); what it never had was a control that *shows* them. This
/// panel is that view: it holds a copy of the heading list (refreshed by
/// [`Self::sync_with_editor`]), paints it as an indented list, and reports the row the reader
/// activated.
///
/// Selection and navigation are deliberately separate. `goto` selects a row and reports the line
/// it addresses; moving the editor's caret is the host's call, made through the editor method it
/// already has. That keeps the panel usable beside a document it did not open.
pub struct MarkdownOutlinePanel {
    base: BaseWidget,
    entries: Vec<MarkdownOutlineEntry>,
    /// The row painted as current, as an index into `entries`.
    active_index: Option<usize>,
    /// The width this panel prefers, reported through `size_hint`.
    width: u32,
    /// The first row the viewport paints.
    first_visible: usize,
    /// Emitted with the outline index when a row is activated.
    pub entry_selected: Signal1<usize>,
}

impl MarkdownOutlinePanel {
    /// Creates an empty panel 180 px wide.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::MarkdownOutlinePanel,
                geometry,
                "MarkdownOutlinePanel",
            ),
            entries: Vec::new(),
            active_index: None,
            width: 180,
            first_visible: 0,
            entry_selected: Signal1::new(),
        }
    }

    /// The headings this panel shows.
    pub fn entries(&self) -> &[MarkdownOutlineEntry] {
        &self.entries
    }

    /// Replaces the entries, dropping an active index that no longer exists.
    pub fn set_entries(&mut self, entries: Vec<MarkdownOutlineEntry>) {
        self.entries = entries;
        if self.active_index.is_some_and(|index| index >= self.entries.len()) {
            self.active_index = None;
        }
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Copies `editor`'s outline into this panel.
    ///
    /// The active row follows the editor's caret: the last heading at or before the caret's line,
    /// which is the heading the reader is currently "inside". Without a preceding heading the
    /// first row is used so a document that starts with body text still has a current row.
    pub fn sync_with_editor(&mut self, editor: &MarkdownEditor) {
        let entries = editor.outline();
        let caret = editor.cursor_line();
        let active = entries
            .iter()
            .rposition(|entry| entry.line <= caret)
            .or_else(|| (!entries.is_empty()).then_some(0));
        self.entries = entries;
        self.active_index = active;
        self.clamp_scroll();
        self.base.request_redraw();
    }

    /// The row painted as current.
    pub fn active_index(&self) -> Option<usize> {
        self.active_index
    }

    /// Marks `index` as current without emitting `entry_selected`.
    ///
    /// This is the editor-to-panel direction (a caret move), which is not a reader action and must
    /// not be reported as one.
    pub fn set_active_index(&mut self, index: Option<usize>) {
        let next = index.filter(|index| *index < self.entries.len());
        if self.active_index == next {
            return;
        }
        self.active_index = next;
        if let Some(index) = next {
            self.reveal(index);
        }
        self.base.request_redraw();
    }

    /// Activates `index`, returning the document line it addresses.
    ///
    /// Returns `None` for a row that does not exist: a caller told about a line it cannot address
    /// would ask the editor to jump somewhere arbitrary.
    pub fn goto(&mut self, index: usize) -> Option<usize> {
        let line = self.entries.get(index)?.line;
        if self.active_index != Some(index) {
            self.active_index = Some(index);
            self.reveal(index);
            self.base.request_redraw();
        }
        self.entry_selected.emit(index);
        Some(line)
    }

    /// The preferred width reported through `size_hint`.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Sets the preferred width.
    pub fn set_width(&mut self, width: u32) {
        let width = width.max(40);
        if self.width == width {
            return;
        }
        self.width = width;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Rows the viewport can paint.
    fn visible_rows(&self) -> usize {
        ((self.geometry().height as i32 / OUTLINE_ROW_HEIGHT).max(1)) as usize
    }

    /// The painted box of row `index`, when it is visible.
    pub fn row_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.entries.len() {
            return None;
        }
        let row = index.checked_sub(self.first_visible)?;
        if row >= self.visible_rows() {
            return None;
        }
        let rect = self.geometry();
        Some(Rect::new(
            rect.x,
            rect.y + row as i32 * OUTLINE_ROW_HEIGHT,
            rect.width,
            OUTLINE_ROW_HEIGHT as u32,
        ))
    }

    /// The deepest indent the panel may draw, so a narrow panel keeps its titles readable.
    fn max_level_indent(&self) -> i32 {
        ((self.geometry().width as i32 - 40) / OUTLINE_INDENT).max(0)
    }

    /// Scrolls so `index` is visible.
    fn reveal(&mut self, index: usize) {
        let rows = self.visible_rows();
        if index < self.first_visible {
            self.first_visible = index;
        } else if index >= self.first_visible + rows {
            self.first_visible = index + 1 - rows;
        }
        self.clamp_scroll();
    }

    /// Keeps the scroll offset inside the entry list.
    fn clamp_scroll(&mut self) {
        let max_first = self.entries.len().saturating_sub(self.visible_rows());
        self.first_visible = self.first_visible.min(max_first);
    }

    /// The entry index under `pos`.
    ///
    /// Public because a host that mounts the panel in a container of its own resolves the press
    /// itself; the editor's mounted panel uses it for the same reason.
    pub fn entry_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains_point(pos) {
            return None;
        }
        let row = (pos.y - rect.y) / OUTLINE_ROW_HEIGHT;
        if row < 0 {
            return None;
        }
        let index = self.first_visible + row as usize;
        (index < self.entries.len()).then_some(index)
    }
}

impl Widget for MarkdownOutlinePanel {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(self.width, 240)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves `entry_selected`; see `DiffViewer`'s note for why the arms are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "entry_selected" => {
                Some(EventSignalRef::mapped("entry_selected", &self.entry_selected, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for MarkdownOutlinePanel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "entries" => Ok(CapabilityValue::Tuple(
                self.entries
                    .iter()
                    .map(|entry| {
                        CapabilityValue::String(format!(
                            "{}|{}|{}",
                            entry.level, entry.title, entry.line
                        ))
                    })
                    .collect(),
            )),
            "active_index" => Ok(match self.active_index {
                Some(index) => CapabilityValue::UInt(index as u64),
                None => CapabilityValue::Null,
            }),
            "width" => Ok(CapabilityValue::UInt(self.width as u64)),
            "entry_count" => Ok(CapabilityValue::UInt(self.entries.len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "entries" => {
                let items = expect_string_list(value)?;
                let mut entries = Vec::with_capacity(items.len());
                for item in items {
                    let mut parts = item.splitn(3, '|');
                    let level = parts
                        .next()
                        .and_then(|text| text.trim().parse::<u8>().ok())
                        .ok_or(CapabilityAccessError::TypeMismatch)?;
                    let title = parts.next().unwrap_or_default().to_string();
                    let line = parts
                        .next()
                        .and_then(|text| text.trim().parse::<usize>().ok())
                        .ok_or(CapabilityAccessError::TypeMismatch)?;
                    entries.push(MarkdownOutlineEntry { line, level: level.clamp(1, 6), title });
                }
                self.set_entries(entries);
                Ok(())
            }
            "active_index" => match value {
                CapabilityValue::Null => {
                    self.set_active_index(None);
                    Ok(())
                }
                CapabilityValue::UInt(index) => {
                    let index =
                        usize::try_from(index).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                    if index >= self.entries.len() {
                        return Err(CapabilityAccessError::OutOfRange);
                    }
                    self.set_active_index(Some(index));
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "width" => {
                let width = u32::try_from(expect_usize(value)?)
                    .map_err(|_| CapabilityAccessError::TypeMismatch)?;
                self.set_width(width);
                Ok(())
            }
            "entry_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["entries", "active_index", "width", "entry_count", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for MarkdownOutlinePanel {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.entry_at(*pos) {
                    let _ = self.goto(index);
                }
            }
            Event::KeyPress { key, .. } => {
                let current = self.active_index.unwrap_or(0);
                match *key {
                    38 => {
                        if !self.entries.is_empty() {
                            let _ = self.goto(current.saturating_sub(1));
                        }
                    }
                    40 => {
                        if !self.entries.is_empty() {
                            let _ = self.goto((current + 1).min(self.entries.len() - 1));
                        }
                    }
                    13 if !self.entries.is_empty() => {
                        let _ = self.goto(current.min(self.entries.len() - 1));
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => {}
        }
    }
}

impl Draw for MarkdownOutlinePanel {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("markdown_editor");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(surface));
        let accent =
            crate::style::semantic_color(crate::style::SemanticColor::Info).unwrap_or(Color::BLUE);
        let font = effective_font(style);

        context.face_with_gradient_corners(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        let rows = self.visible_rows();
        for offset in 0..rows {
            let index = self.first_visible + offset;
            let Some(entry) = self.entries.get(index) else {
                break;
            };
            let Some(row) = self.row_rect(index) else {
                break;
            };
            if self.active_index == Some(index) {
                context.fill_rect(row, surface.blend(&accent, 0.20));
            }
            let indent = (entry.level.saturating_sub(1) as i32 * OUTLINE_INDENT)
                .min(self.max_level_indent());
            let title = crate::widget::text_utils::elide_end(
                &entry.title,
                ((row.width as i32 - indent - 8) / 7).max(1) as usize,
            );
            context.draw_text(
                Point::new(row.x + 6 + indent, row.y + 4),
                &title,
                font,
                ink,
                HorizontalAlignment::Left,
            );
        }
    }
}
