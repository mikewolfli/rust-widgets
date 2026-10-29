// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// The header control of a kanban column.
///
/// The board paints its column titles as part of its own chrome, so a column could not be renamed,
/// its WIP limit could not be read, and its collapse chevron was a painted triangle with no
/// target. This control is that header as a widget: title, count badge, WIP limit, collapse
/// and an add-card affordance, each of which reports the gesture it received.
pub struct KanbanColumnHeader {
    base: BaseWidget,
    title: String,
    count: usize,
    wip_limit: Option<usize>,
    collapsed: bool,
    editable: bool,
    /// The in-progress title edit, when the header is being renamed.
    edit_buffer: Option<String>,
    /// The badge label, with the `(count, wip_limit)` pair it was built for (BK-P12).
    badge_text_cache: (usize, Option<usize>, String),
    /// The count-only label the badge width is measured from, with the count it was built for.
    count_text_cache: (usize, String),
    /// Emitted with the new title after a rename.
    pub column_renamed: Signal1<String>,
    /// Emitted with the new collapsed flag.
    pub column_collapsed_changed: Signal1<bool>,
    /// Emitted when the add-card button is pressed.
    pub add_card_requested: GenericSignal,
}

impl KanbanColumnHeader {
    /// Creates a header showing `title`.
    pub fn new(geometry: Rect, title: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::KanbanColumnHeader, geometry, "KanbanColumnHeader"),
            title: title.into(),
            count: 0,
            wip_limit: None,
            collapsed: false,
            editable: true,
            edit_buffer: None,
            badge_text_cache: (0, None, String::from("0")),
            count_text_cache: (0, String::from("0")),
            column_renamed: Signal1::new(),
            column_collapsed_changed: Signal1::new(),
            add_card_requested: GenericSignal::new(),
        }
    }

    /// The column title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Renames the column, emitting `column_renamed`.
    ///
    /// Returns whether the title changed; a rename to the same text is not reported, because a
    /// subscriber that persisted the column would write a change that did not happen.
    pub fn rename(&mut self, title: impl Into<String>) -> bool {
        let title = title.into();
        if self.title == title {
            return false;
        }
        self.title = title.clone();
        self.column_renamed.emit(title);
        self.base.request_redraw();
        true
    }

    /// The card count shown in the badge.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Sets the card count shown in the badge.
    pub fn set_count(&mut self, count: usize) {
        if self.count == count {
            return;
        }
        self.count = count;
        self.base.request_redraw();
    }

    /// The column's work-in-progress limit.
    pub fn wip_limit(&self) -> Option<usize> {
        self.wip_limit
    }

    /// Sets the work-in-progress limit.
    pub fn set_wip_limit(&mut self, limit: Option<usize>) {
        if self.wip_limit == limit {
            return;
        }
        self.wip_limit = limit;
        self.base.request_redraw();
    }

    /// Whether the column is collapsed.
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Collapses or expands the column, emitting `column_collapsed_changed`.
    pub fn set_collapsed(&mut self, collapsed: bool) {
        if self.collapsed == collapsed {
            return;
        }
        self.collapsed = collapsed;
        self.column_collapsed_changed.emit(collapsed);
        self.base.request_redraw();
    }

    /// Whether the title may be edited by the reader.
    pub fn is_editable(&self) -> bool {
        self.editable
    }

    /// Enables or disables title editing.
    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        if !editable {
            self.edit_buffer = None;
        }
        self.base.request_redraw();
    }

    /// Whether the count has reached the WIP limit.
    pub fn is_at_wip_limit(&self) -> bool {
        self.wip_limit.is_some_and(|limit| self.count >= limit)
    }

    /// Whether the title is being edited.
    pub fn is_editing(&self) -> bool {
        self.edit_buffer.is_some()
    }

    /// Starts editing the title from the current text.
    pub fn begin_title_edit(&mut self) -> bool {
        if !self.editable || self.edit_buffer.is_some() {
            return false;
        }
        self.edit_buffer = Some(self.title.clone());
        self.base.request_redraw();
        true
    }

    /// Commits the in-progress rename.
    pub fn commit_title_edit(&mut self) -> bool {
        let Some(next) = self.edit_buffer.take() else {
            return false;
        };
        if next == self.title {
            self.base.request_redraw();
            return false;
        }
        let changed = self.rename(next);
        self.base.request_redraw();
        changed
    }

    /// Cancels the in-progress rename.
    pub fn cancel_title_edit(&mut self) -> bool {
        let canceled = self.edit_buffer.take().is_some();
        if canceled {
            self.base.request_redraw();
        }
        canceled
    }

    /// The painted box of the collapse chevron.
    pub fn collapse_rect(&self) -> Rect {
        let rect = self.base.geometry();
        Rect::new(
            rect.x + 4,
            rect.y + (rect.height as i32 - HEADER_ICON) / 2,
            HEADER_ICON as u32,
            HEADER_ICON as u32,
        )
    }

    /// The painted box of the add-card button.
    pub fn add_rect(&self) -> Rect {
        let rect = self.base.geometry();
        Rect::new(
            rect.x + rect.width as i32 - HEADER_ICON - 4,
            rect.y + (rect.height as i32 - HEADER_ICON) / 2,
            HEADER_ICON as u32,
            HEADER_ICON as u32,
        )
    }
}

impl Widget for KanbanColumnHeader {
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
        crate::core::Size::new(220, HEADER_SIDE as u32)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves the header's three events; see `DiffViewer`'s note for why the arms are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "column_renamed" => {
                Some(EventSignalRef::mapped("column_renamed", &self.column_renamed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "column_collapsed_changed" => Some(EventSignalRef::mapped(
                "column_collapsed_changed",
                &self.column_collapsed_changed,
                |v| CapabilityValue::Bool(*v),
            )),
            "add_card_requested" => {
                Some(EventSignalRef::unit("add_card_requested", &self.add_card_requested))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for KanbanColumnHeader {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "count" => Ok(CapabilityValue::UInt(self.count as u64)),
            "wip_limit" => Ok(match self.wip_limit {
                Some(limit) => CapabilityValue::UInt(limit as u64),
                None => CapabilityValue::Null,
            }),
            "collapsed" => Ok(CapabilityValue::Bool(self.collapsed)),
            "editable" => Ok(CapabilityValue::Bool(self.editable)),
            "at_wip_limit" => Ok(CapabilityValue::Bool(self.is_at_wip_limit())),
            "editing" => Ok(CapabilityValue::Bool(self.is_editing())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.rename(expect_string(value)?);
                Ok(())
            }
            "count" => {
                let count = match value {
                    CapabilityValue::UInt(count) => {
                        usize::try_from(count).map_err(|_| CapabilityAccessError::OutOfRange)?
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_count(count);
                Ok(())
            }
            "wip_limit" => match value {
                CapabilityValue::Null => {
                    self.set_wip_limit(None);
                    Ok(())
                }
                CapabilityValue::UInt(limit) => {
                    self.set_wip_limit(Some(
                        usize::try_from(limit).map_err(|_| CapabilityAccessError::OutOfRange)?,
                    ));
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "collapsed" => {
                self.set_collapsed(expect_bool(value)?);
                Ok(())
            }
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            "at_wip_limit" | "editing" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "count",
            "wip_limit",
            "collapsed",
            "editable",
            "at_wip_limit",
            "editing",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for KanbanColumnHeader {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if self.edit_buffer.is_some() {
            match event {
                Event::TextInput { text } | Event::ImeCommit { text } => {
                    if let Some(buffer) = self.edit_buffer.as_mut() {
                        buffer.extend(text.chars().filter(|character| !character.is_control()));
                    }
                    self.base.request_redraw();
                }
                Event::KeyPress { key, .. } => match *key {
                    13 => {
                        let _ = self.commit_title_edit();
                    }
                    27 => {
                        self.cancel_title_edit();
                    }
                    8 => {
                        if let Some(buffer) = self.edit_buffer.as_mut() {
                            buffer.pop();
                        }
                        self.base.request_redraw();
                    }
                    _ => { /* Other keys are not relevant */ }
                },
                _ => {}
            }
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if self.collapse_rect().contains_point(*pos) {
                    self.set_collapsed(!self.collapsed);
                } else if self.add_rect().contains_point(*pos) {
                    self.add_card_requested.emit();
                }
            }
            Event::MouseDoubleClick { pos, button: 1, .. }
                if self.editable && self.base.geometry().contains_point(*pos) =>
            {
                let _ = self.begin_title_edit();
            }
            _ => {}
        }
    }
}

impl Draw for KanbanColumnHeader {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("kanban_board");
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
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        let accent = crate::style::palette_shared()
            .as_ref()
            .map(|colors| colors.primary)
            .unwrap_or(Color::PRIMARY);
        let warning = crate::style::semantic_color(crate::style::SemanticColor::Warning)
            .unwrap_or(Color::PRIMARY);
        let font = role_font(style, 12.0);

        context.face_with_gradient_corners(
            rect,
            surface.blend(&ink, 0.05),
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

        let chevron = if self.collapsed { IconName::ChevronRight } else { IconName::ChevronDown };
        crate::widget::draw_icon_at(context, self.collapse_rect(), ink, chevron);

        let title = match &self.edit_buffer {
            Some(buffer) => buffer.as_str(),
            None => self.title.as_str(),
        };
        let add = self.add_rect();
        // Both labels are rebuilt only when the pair they spell changes. The width is still
        // measured from the count alone, exactly as before, so the frame is unchanged.
        if self.badge_text_cache.0 != self.count || self.badge_text_cache.1 != self.wip_limit {
            self.badge_text_cache.0 = self.count;
            self.badge_text_cache.1 = self.wip_limit;
            let badge_text = &mut self.badge_text_cache.2;
            badge_text.clear();
            if let Some(limit) = self.wip_limit {
                let _ = write!(badge_text, "{}/{}", self.count, limit);
            } else {
                let _ = write!(badge_text, "{}", self.count);
            }
        }
        if self.count_text_cache.0 != self.count {
            self.count_text_cache.0 = self.count;
            let count_text = &mut self.count_text_cache.1;
            count_text.clear();
            let _ = write!(count_text, "{}", self.count);
        }
        let badge_width = context.measure_text(&self.count_text_cache.1, &font).width as i32 + 10;
        let text_left = self.collapse_rect().x + HEADER_ICON + 6;
        let text_right = (add.x - 6 - badge_width).max(text_left + 8);
        let text = crate::widget::text_utils::elide_end(
            title,
            ((text_right - text_left).max(8) / 7) as usize,
        );
        context.draw_text(
            Point::new(text_left, rect.y + (rect.height as i32 - 12) / 2),
            &text,
            &font,
            ink,
            HorizontalAlignment::Left,
        );

        let badge =
            Rect::new(text_right, rect.y + (rect.height as i32 - 16) / 2, badge_width as u32, 16);
        context.fill_rounded_rect(badge, 8, surface.blend(&ink, 0.12));
        context.draw_text(
            Point::new(badge.x + 5, badge.y + 2),
            &self.badge_text_cache.2,
            &font,
            if self.is_at_wip_limit() { warning } else { ink },
            HorizontalAlignment::Left,
        );

        crate::widget::draw_icon_at(context, add, ink.blend(&surface, 0.25), IconName::Plus);
        // The accent is read for the at-limit outline so a theme's warning colour is the one thing
        // the reader sees when a column is full; the stroke sits inside the badge.
        if self.is_at_wip_limit() {
            context.draw_rect(badge, warning.blend(&accent, 0.2));
        }
    }
}
