// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// Padding inside a card, and the gap between its stacked lines.
const CARD_INSET: i32 = 8;
/// Side of a card's done box.
const CARD_DONE_BOX: i32 = 14;
/// Height of a column header strip and the side of its icon cells.
pub(super) const HEADER_SIDE: i32 = 32;
pub(super) const HEADER_ICON: i32 = 14;

/// The card control of a kanban board.
///
/// # Why this is a control and not the board's row
///
/// [`KanbanBoard`] paints each card as a rectangle from its own model. That is enough to drag a
/// card, and not enough for anything else: there is no child control to hit the done box, root an
/// editor for the title, or carry tags and an assignee that a real board shows. This widget is
/// that card as a control in its own right; the board's [`KanbanCard`] stays the data model.
///
/// The name is deliberately distinct: `KanbanCard` already meant the data (id, title, description,
/// done) and reusing it for the control would have broken every existing caller for a rename that
/// adds no behaviour.
pub struct KanbanCardView {
    base: BaseWidget,
    title: String,
    description: String,
    done: bool,
    tags: Vec<String>,
    assignee: String,
    draggable: bool,
    /// The in-progress title edit, when the card is being renamed.
    edit_buffer: Option<String>,
    /// Emitted when the card body is clicked (not its done box).
    pub card_clicked: GenericSignal,
    /// Emitted with the committed title after an edit.
    pub card_edited: Signal1<String>,
    /// Emitted with the new done flag when it is toggled.
    pub card_done_toggled: Signal1<bool>,
}

impl KanbanCardView {
    /// Creates a card with `title` and no other content.
    pub fn new(geometry: Rect, title: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::KanbanCardView, geometry, "KanbanCardView"),
            title: title.into(),
            description: String::new(),
            done: false,
            tags: Vec::new(),
            assignee: String::new(),
            draggable: true,
            edit_buffer: None,
            card_clicked: GenericSignal::new(),
            card_edited: Signal1::new(),
            card_done_toggled: Signal1::new(),
        }
    }

    /// The card title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the title without starting an edit, reporting it as a card edit.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if self.title == title {
            return;
        }
        self.title = title.clone();
        self.card_edited.emit(title);
        self.base.request_redraw();
    }

    /// The description line.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Sets the description line.
    pub fn set_description(&mut self, description: impl Into<String>) {
        let description = description.into();
        if self.description == description {
            return;
        }
        self.description = description;
        self.base.request_redraw();
    }

    /// Whether the card is marked done.
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Sets the done flag, emitting `card_done_toggled`.
    pub fn set_done(&mut self, done: bool) {
        if self.done == done {
            return;
        }
        self.done = done;
        self.card_done_toggled.emit(done);
        self.base.request_redraw();
    }

    /// The card's tags, in display order.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Replaces the tags.
    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.tags = tags;
        self.base.request_redraw();
    }

    /// The assignee line.
    pub fn assignee(&self) -> &str {
        &self.assignee
    }

    /// Sets the assignee line.
    pub fn set_assignee(&mut self, assignee: impl Into<String>) {
        self.assignee = assignee.into();
        self.base.request_redraw();
    }

    /// Whether a board may drag this card.
    pub fn is_draggable(&self) -> bool {
        self.draggable
    }

    /// Enables or disables dragging for this card.
    pub fn set_draggable(&mut self, draggable: bool) {
        self.draggable = draggable;
    }

    /// Whether the title is being edited.
    pub fn is_editing(&self) -> bool {
        self.edit_buffer.is_some()
    }

    /// Starts editing the title from the current text.
    pub fn begin_title_edit(&mut self) -> bool {
        if self.edit_buffer.is_some() {
            return false;
        }
        self.edit_buffer = Some(self.title.clone());
        self.base.request_redraw();
        true
    }

    /// Commits the in-progress title edit.
    pub fn commit_title_edit(&mut self) -> bool {
        let Some(next) = self.edit_buffer.take() else {
            return false;
        };
        self.base.request_redraw();
        if next == self.title {
            return false;
        }
        self.title = next.clone();
        self.card_edited.emit(next);
        true
    }

    /// Cancels the in-progress title edit, restoring the old title.
    pub fn cancel_title_edit(&mut self) -> bool {
        self.edit_buffer.take().is_some()
    }

    /// The painted box of the done box.
    pub fn done_box_rect(&self) -> Rect {
        let rect = self.base.geometry();
        Rect::new(
            rect.x + CARD_INSET,
            rect.y + CARD_INSET,
            CARD_DONE_BOX as u32,
            CARD_DONE_BOX as u32,
        )
    }
}

impl Widget for KanbanCardView {
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
        crate::core::Size::new(220, 72)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves the card's three events; see `DiffViewer`'s note for why the arms are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "card_clicked" => Some(EventSignalRef::unit("card_clicked", &self.card_clicked)),
            "card_edited" => Some(EventSignalRef::mapped("card_edited", &self.card_edited, |v| {
                CapabilityValue::String(v.clone())
            })),
            "card_done_toggled" => {
                Some(EventSignalRef::mapped("card_done_toggled", &self.card_done_toggled, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for KanbanCardView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "description" => Ok(CapabilityValue::String(self.description.clone())),
            "done" => Ok(CapabilityValue::Bool(self.done)),
            "tags" => Ok(CapabilityValue::Tuple(
                self.tags.iter().map(|tag| CapabilityValue::String(tag.clone())).collect(),
            )),
            "assignee" => Ok(CapabilityValue::String(self.assignee.clone())),
            "draggable" => Ok(CapabilityValue::Bool(self.draggable)),
            "editing" => Ok(CapabilityValue::Bool(self.is_editing())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "description" => {
                self.set_description(expect_string(value)?);
                Ok(())
            }
            "done" => {
                self.set_done(expect_bool(value)?);
                Ok(())
            }
            "tags" => {
                let tags = expect_string_list(value)?;
                self.set_tags(tags);
                Ok(())
            }
            "assignee" => {
                self.set_assignee(expect_string(value)?);
                Ok(())
            }
            "draggable" => {
                self.set_draggable(expect_bool(value)?);
                Ok(())
            }
            "editing" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "description",
            "done",
            "tags",
            "assignee",
            "draggable",
            "editing",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for KanbanCardView {
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
                        self.base.request_redraw();
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
                if self.done_box_rect().contains_point(*pos) {
                    self.set_done(!self.done);
                } else if self.base.geometry().contains_point(*pos) {
                    self.card_clicked.emit();
                }
            }
            Event::MouseDoubleClick { pos, button: 1, .. }
                if self.base.geometry().contains_point(*pos) =>
            {
                let _ = self.begin_title_edit();
            }
            _ => {}
        }
    }
}

impl Draw for KanbanCardView {
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
        let font = role_font(style, 12.0);
        let small = role_font(style, 10.0);

        context.fill_rounded_rect(rect, 6, surface);
        context.draw_rounded_rect_stroke(rect, 6, border, 1);

        let done_box = self.done_box_rect();
        context.draw_rect(done_box, border);
        if self.done {
            crate::widget::draw_icon_at(context, done_box, accent, IconName::Check);
        }

        let text_x = done_box.x + done_box.width as i32 + 6;
        let title = match &self.edit_buffer {
            Some(buffer) => buffer.as_str(),
            None => self.title.as_str(),
        };
        let strike = if self.done { ink.blend(&surface, 0.45) } else { ink };
        let fitted = crate::widget::text_utils::elide_end(
            title,
            ((rect.width as i32 - (text_x - rect.x) - CARD_INSET) / 7).max(1) as usize,
        );
        context.draw_text(
            Point::new(text_x, rect.y + CARD_INSET),
            &fitted,
            &font,
            strike,
            HorizontalAlignment::Left,
        );
        if self.done {
            let width = context.measure_text(&fitted, &font).width as i32;
            context.draw_line(
                Point::new(text_x, rect.y + CARD_INSET + 7),
                Point::new(text_x + width, rect.y + CARD_INSET + 7),
                strike,
            );
        }

        let mut y = rect.y + CARD_INSET + 18;
        if !self.description.is_empty() {
            let description = crate::widget::text_utils::elide_end(
                &self.description,
                ((rect.width as i32 - CARD_INSET * 2) / 6).max(1) as usize,
            );
            context.draw_text(
                Point::new(rect.x + CARD_INSET, y),
                &description,
                &small,
                ink.blend(&surface, 0.30),
                HorizontalAlignment::Left,
            );
            y += 16;
        }
        if !self.tags.is_empty() {
            let mut x = rect.x + CARD_INSET;
            for tag in &self.tags {
                let label =
                    crate::widget::text_utils::elide_end(tag, (rect.width as usize / 7).max(2));
                let width = context.measure_text(&label, &small).width + 10;
                if x + width as i32 > rect.x + rect.width as i32 - CARD_INSET {
                    break;
                }
                let chip = Rect::new(x, y, width, 14);
                context.fill_rounded_rect(chip, 4, surface.blend(&accent, 0.22));
                context.draw_text(
                    Point::new(x + 4, y + 1),
                    &label,
                    &small,
                    ink,
                    HorizontalAlignment::Left,
                );
                x += width as i32 + 4;
            }
        }
        if !self.assignee.is_empty() {
            let label = crate::widget::text_utils::elide_end(&self.assignee, 12);
            let width = context.measure_text(&label, &small).width as i32;
            context.draw_text(
                Point::new(
                    rect.x + rect.width as i32 - width - CARD_INSET,
                    rect.y + rect.height as i32 - 18,
                ),
                &label,
                &small,
                ink.blend(&surface, 0.30),
                HorizontalAlignment::Left,
            );
        }
    }
}
