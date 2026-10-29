// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `TextArea`'s runtime contracts: the pointer gesture state machine, the `Widget`,
//! `WidgetProperties` and `EventHandler` implementations, and drag/drop.
//!
//! Every one of these is a contract the host calls into, so they sit together rather than beside
//! the document model they mutate.

use crate::compat::ToString;
use crate::core::{Point, Size};
use crate::event::dnd::{DragPayload, DragSource, DropEffect, DropTarget};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::widget::capability::coercion::{
    expect_bool, expect_selection_range, expect_string, expect_usize,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{active_profile, dimensions, profile_metrics};
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::widget_trait::InputMethodQuery;
use crate::widget::{BaseWidget, Widget};
use crate::{impl_widget_property_hooks, property_names_of};

use super::model::TextArea;
use super::paint::{FIRST_LINE_INSET, LINE_H};

impl TextArea {
    /// A primary press at `pos`: grabs the scrollbar when the point is on it, otherwise focuses
    /// the area, starts the caret blink and places the caret / anchors the selection.
    ///
    /// Shared by the mouse and touch press arms, so a finger enters exactly the state machine a
    /// pointer does instead of a second copy of it.
    fn press_with_pointer(&mut self, pos: Point, modifiers: u32) {
        // I-118: a press on the scrollbar thumb grabs it instead of moving the caret.
        if self.scrollbar_contains(pos) {
            self.scrollbar_drag = true;
            self.scrollbar_scroll_to(pos.y);
            self.request_redraw();
            return;
        }
        self.focused = true;
        self.cursor_blink.start();
        let index = self.byte_index_at(pos);
        self.press_at(index, crate::shortcut::Modifiers::from_event_bits(modifiers));
    }

    /// Starts a pointer selection at the character `index`, extending from the anchor when `Shift`
    /// is held.
    ///
    /// # Why a press must not normalise
    ///
    /// A press landing exactly on the caret produces a zero-width anchor/caret pair, and collapsing
    /// that is right for a *result* — but this is the *start* of a gesture, and the anchor is what
    /// the following drag grows from. Normalising here would delete the anchor the press just set,
    /// so dragging back over the value would select nothing. The gesture's end normalises instead,
    /// in [`Self::end_drag`].
    pub fn press_at(&mut self, index: usize, modifiers: crate::shortcut::Modifiers) {
        let index = floor_char_boundary(&self.text, index.min(self.text.len()));
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            // Extend: keep the anchor (adopting the caret when the gesture is new) and move the
            // caret to where the pointer went down. Leaving the caret alone would make a
            // shift-press a no-op that only re-anchored, so shift-clicking to the other end of a
            // line would select nothing.
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor_pos);
            }
            self.cursor_pos = index;
        } else {
            // A plain press starts a new gesture, anchored where it landed.
            self.selection_anchor = Some(index);
            self.cursor_pos = index;
        }
        self.dragging_selection = true;
        self.notify_caret_change();
        self.base.request_redraw();
    }

    /// Extends an in-progress pointer selection to the character at `index`.
    ///
    /// Only a gesture that began with [`Self::press_at`] continues here; a bare hover would
    /// otherwise move a caret the user never grabbed.
    pub fn drag_to(&mut self, index: usize) {
        if !self.dragging_selection {
            return;
        }
        self.cursor_pos = floor_char_boundary(&self.text, index.min(self.text.len()));
        self.normalize_selection();
        self.notify_caret_change();
        self.base.request_redraw();
    }

    /// Ends a pointer selection, collapsing a gesture that never moved. Returns whether one was in
    /// progress.
    pub fn end_drag(&mut self) -> bool {
        let was_dragging = core::mem::replace(&mut self.dragging_selection, false);
        if was_dragging {
            self.normalize_selection();
            self.notify_caret_change();
        }
        was_dragging
    }

    /// Whether `pos` is inside the control's painted rectangle.
    ///
    /// A text area paints its whole `geometry()` — unlike a single-line field, whose content is a
    /// centred band — so the hit test is the control's own rectangle and no second box can drift
    /// from the ink.
    fn hit_test(&self, pos: Point) -> bool {
        self.geometry().contains_point(pos)
    }

    /// The byte offset the point `pos` falls on.
    ///
    /// # How a pixel becomes an offset
    ///
    /// The line is whichever [`LINE_H`] band `y` lands in — that is a layout choice, and the grid is
    /// what the paint path advances by, so the two agree. The **column** within the line is measured
    /// rather than divided: [`Self::column_at_x`] walks the line and measures each prefix, which is
    /// the only way a click can land on the character the user is pointing at when the face's glyphs
    /// have different widths.
    ///
    /// The result is clamped to the end of the *line* it resolved to, not to the end of the value:
    /// clicking to the right of a short line must put the caret at that line's end, not on a later
    /// line. The final snap to a character boundary keeps `&text[..caret]` total on multi-byte text.
    fn byte_index_at(&self, pos: Point) -> usize {
        let rect = self.geometry();
        let origin_x = rect.x + dimensions::TEXT_FIELD_PADDING_H as i32;
        let text_top = rect.y + FIRST_LINE_INSET + (LINE_H - self.line_box_height()) / 2;

        let row = ((pos.y - text_top).max(0) / LINE_H) as usize;
        // The viewport row is translated to the display row it paints, so a click after scrolling
        // resolves to the row the user sees there rather than to the document's own nth row.
        let x = pos.x - origin_x + self.scroll_x - self.content_margin;

        // Walk the table the paint path also reads, so the answer is the same row the glyphs are
        // drawn on rather than a second model of where the newlines are.
        let Some(row) = self.display_row(row) else {
            return self.text.len();
        };
        let start = row.start;
        let content_end = row.end.min(self.text.len());
        start + self.column_at_x(start, content_end, x)
    }
}

impl Widget for TextArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Reports the caret an IME candidate window should anchor to while this area is focused.
    ///
    /// The caret's row is the one the paint path draws it on — the document line, shifted by the
    /// viewport's first visible line — and its x is the measured advance of the text before the
    /// caret on that line. The default `None` is still what every non-editable widget answers.
    fn input_method_query(&self) -> Option<InputMethodQuery> {
        Some(self.caret_query())
    }

    /// Turns a context-menu request into this area's signal; the host supplies the menu.
    fn context_menu_event(&mut self, pos: Point) -> bool {
        self.context_menu_requested.emit(pos);
        true
    }

    /// Opts the area into the runtime's drag coordinator as a source.
    fn drag_source_mut(&mut self) -> Option<&mut dyn DragSource> {
        Some(self)
    }

    /// Opts the area into the runtime's drag coordinator as a drop target.
    fn drop_target_mut(&mut self) -> Option<&mut dyn DropTarget> {
        Some(self)
    }

    fn size_hint(&self) -> Size {
        let line_count = if self.text.is_empty() {
            1
        } else {
            self.text.chars().filter(|&c| c == '\n').count() + 1
        };
        let max_line_width = self.text.lines().map(|l| l.len() as u32).max().unwrap_or(0);
        let w = (max_line_width * 8 + 10).max(120);
        // A text area is a stack of line rows, so its content height grows with the text. The
        // *floor* under that stack is the active profile's text-field height: Material's own
        // historical minimum is 60 px (the control's three-row budget), while a compact profile
        // such as macOS lets an empty area start at its shorter field height. The profile never
        // makes the area taller than the text already asks for, so multi-line content is
        // unchanged on every profile.
        let field_height = profile_metrics(active_profile()).text_field_min_height;
        let material_field_height =
            profile_metrics(crate::widget::metrics::ControlMetricsProfile::Material)
                .text_field_min_height;
        let floor = if field_height < material_field_height { field_height } else { 60 };
        let h = (line_count as u32 * LINE_H as u32 + 10).max(floor);
        Size::new(w, h)
    }

    /// Advances the caret blink and reports whether another frame is owed (I-116).
    fn tick(&mut self, delta_ms: u32) -> bool {
        TextArea::tick(self, delta_ms)
    }

    /// A focused area keeps a blinking caret, so the host must keep scheduling frames (I-116).
    fn is_animating(&self) -> bool {
        self.focused
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "changed" => Some(EventSignalRef::unit("changed", &self.changed)),
            "selection_changed" => {
                Some(EventSignalRef::unit("selection_changed", &self.selection_changed))
            }
            "cursor_position_changed" => Some(EventSignalRef::mapped(
                "cursor_position_changed",
                &self.cursor_position_changed,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            "undo_available" => {
                Some(EventSignalRef::mapped("undo_available", &self.undo_available, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "redo_available" => {
                Some(EventSignalRef::mapped("redo_available", &self.redo_available, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| CapabilityValue::String(format!("{v:?}")),
            )),
            _ => None,
        }
    }
}

/// A selected run of text is the area's drag payload.
///
/// The item id and label both hold the text because [`DragPayload`] carries an opaque
/// identifier rather than a typed body; for `text/plain` the identifier *is* the content, and
/// using the same value for both fields means a target can read either without a rule about
/// which one is authoritative.
impl DragSource for TextArea {
    fn drag_payload(&self, pos: Point) -> Option<DragPayload> {
        if !self.hit_test(pos) {
            return None;
        }
        let selected = self.selected_text();
        if selected.is_empty() {
            return None;
        }
        Some(DragPayload::new("text/plain", selected.clone()).with_label(selected).with_origin(pos))
    }

    /// A `Move` drop removes the source selection; every other effect keeps it.
    fn on_drag_finished(&mut self, effect: DropEffect) {
        if !effect.consumes_source() || self.read_only {
            return;
        }
        let before = self.text.clone();
        if self.replace_selection() {
            self.line_spans_cache.borrow_mut().take();
            self.pending_line_edit.set(None);
            self.record_edit(before);
            self.base.request_redraw();
        }
    }
}

/// The area accepts plain text and inserts it at the drop point.
impl DropTarget for TextArea {
    fn can_accept(&self, payload: &DragPayload) -> bool {
        !self.read_only && payload.is_type("text/plain")
    }

    fn on_drop(&mut self, payload: &DragPayload, pos: Point) -> DropEffect {
        if !self.can_accept(payload) || !self.hit_test(pos) {
            return DropEffect::None;
        }
        let text =
            if payload.label.is_empty() { payload.item_id.clone() } else { payload.label.clone() };
        if text.is_empty() {
            return DropEffect::None;
        }
        self.cursor_pos = self.byte_index_at(pos);
        self.selection_anchor = None;
        self.insert_str(&text);
        self.base.request_redraw();
        DropEffect::Copy
    }
}

/// `TextArea`'s property contract.
impl WidgetProperties for TextArea {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            "read_only" => Ok(CapabilityValue::Bool(self.is_read_only())),
            "max_length" => Ok(CapabilityValue::UInt(self.max_length() as u64)),
            // The published selection pair is what `set_widget_selection` / `widget_selection`
            // speak (API-03); `selection_range` is the single derivation every other reader uses.
            "selection" => Ok(match self.selection_range() {
                Some((start, end)) => {
                    CapabilityValue::String(crate::compat::format!("{start},{end}"))
                }
                None => CapabilityValue::Null,
            }),
            "soft_wrap" => Ok(CapabilityValue::Bool(self.soft_wrap())),
            "content_margin" => Ok(CapabilityValue::UInt(self.content_margin().max(0) as u64)),
            "max_block_count" => match self.max_block_count() {
                Some(max) => Ok(CapabilityValue::UInt(max as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "scroll_x" => Ok(CapabilityValue::Int(self.scroll_x as i64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "placeholder" => {
                self.set_placeholder(expect_string(value)?);
                Ok(())
            }
            "read_only" => {
                self.set_read_only(expect_bool(value)?);
                Ok(())
            }
            "max_length" => {
                self.set_max_length(expect_usize(value)?);
                Ok(())
            }
            "selection" => {
                // `Null` is the getter's "nothing selected" answer and the schema default, so
                // the write-back round trip accepts it by clearing the range (API-03).
                if matches!(value, CapabilityValue::Null) {
                    self.clear_selection();
                    return Ok(());
                }
                let (start, end) = expect_selection_range(value)?;
                let len = self.text.len();
                let start = floor_char_boundary(&self.text, start.min(len));
                let end = floor_char_boundary(&self.text, end.min(len));
                self.selection_anchor = Some(start);
                self.cursor_pos = end;
                self.notify_caret_change();
                self.base.request_redraw();
                Ok(())
            }
            "soft_wrap" => {
                self.set_soft_wrap(expect_bool(value)?);
                Ok(())
            }
            "content_margin" => {
                self.set_content_margin(expect_usize(value)? as i32);
                Ok(())
            }
            "max_block_count" => {
                match value {
                    CapabilityValue::Null => self.set_max_block_count(None),
                    other => self.set_max_block_count(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "scroll_x" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `TEXT_AREA_PROPERTIES`.
        property_names_of![
            "text",
            "placeholder",
            "read_only",
            "max_length",
            "selection",
            "soft_wrap",
            "content_margin",
            "max_block_count",
            "scroll_x",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `text_area` publishes.
    ///
    /// `insert` and `delete_char` are the editing actions: they act at the caret, which
    /// the control owns, so they take no argument. On a fresh control `delete_char` is a
    /// no-op (the caret is already at position 0), but the command still ran, so it
    /// answers `Ok(())` rather than manufacturing an error for a legal invocation.
    ///
    /// `set_text`, `set_placeholder` and `set_read_only` assign state and need a
    /// payload, so they are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "insert" => {
                self.insert_char(' ');
                Ok(())
            }
            "delete_char" => {
                self.delete_char();
                Ok(())
            }
            "set_text" | "set_placeholder" | "set_read_only" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for TextArea {
    fn handle_event(&mut self, event: &Event) {
        // A queued find/replace intent is a shared write, applied before the event is routed so the
        // frame that follows the strip click already reflects it (COMP-A8).
        #[cfg(full_widgets)]
        self.poll_find_replace_bar();
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::FocusGained { .. } => {
                self.focused = true;
                // I-116: start the caret's blink cycle on focus.
                self.cursor_blink.start();
                self.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.cursor_blink.stop();
                self.request_redraw();
            }
            // Platform-committed text (D09-INPUT-01), routed into the ordinary insert path. A
            // read-only area is rejected here, exactly as the `KeyPress` printable arm is, and the
            // `is_enabled` guard above already ran.
            Event::TextInput { text } => {
                if self.read_only {
                    return;
                }
                self.insert_committed_text(text);
            }
            // A commit ends the composition as well as inserting its text. Clearing the preedit
            // first keeps the underlined preview from surviving the string it previewed: an input
            // method may deliver the commit before its `preedit_end`, and the area would otherwise
            // paint both the committed text and a stale composition.
            Event::ImeCommit { text } => {
                if self.read_only {
                    return;
                }
                self.composition = None;
                self.insert_committed_text(text);
            }
            // The uncommitted composition belongs to the area while the user is building it: the
            // value does not change, but the inline preedit at the caret does.
            Event::ImePreedit { text, .. } => {
                if self.read_only {
                    return;
                }
                self.set_composition(text);
            }
            // Wheel: a positive `delta.y` moves the content down, so the viewport moves toward
            // the start of the document. `WheelScroll` converts the notch count to lines with the
            // desktop default of three lines per notch.
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
                if self.scroll_lines(-scroll.lines_y) {
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, modifiers } => {
                // The event carries the framework's wire bitmask; translate it once, here, so the
                // rest of this handler speaks `Modifiers` and nothing below re-derives a bit.
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = mods.contains(crate::shortcut::Modifiers::SHIFT);
                let primary = crate::shortcut::event_carries_primary(*modifiers);

                // I-117: select-all and copy remain available on a read-only area; undo/redo are
                // mutations and therefore sit behind the guard below.
                if primary && *key == 65 {
                    // Primary+A: select all. Handled on the control that owns the text, so a host
                    // whose window-level "select all" means something else does not swallow it.
                    self.select_all();
                    return;
                }
                if primary && *key == 67 {
                    // Primary+C: copy the selection. Allowed while read-only.
                    let _ = self.copy_selection_to_clipboard();
                    return;
                }
                if self.read_only {
                    return;
                }
                if primary && *key == 90 {
                    // Primary+Z undoes; Primary+Shift+Z is the other conventional redo chord.
                    if shift {
                        let _ = self.redo();
                    } else {
                        let _ = self.undo();
                    }
                    return;
                }
                if primary && *key == 89 {
                    let _ = self.redo();
                    return;
                }
                if primary && *key == 88 {
                    // Primary+X: copy, then delete the selection.
                    let _ = self.copy_selection_to_clipboard();
                    self.delete_char();
                    return;
                }
                if primary && *key == 86 {
                    // Primary+V: paste at the caret, replacing any selection.
                    self.paste_from_clipboard();
                    return;
                }
                match *key {
                    8 => {
                        // Backspace — one character back, or the whole selection.
                        self.delete_char();
                        self.request_redraw();
                    }
                    46 => {
                        // Delete — one character forward, or the whole selection.
                        self.delete_forward();
                        self.request_redraw();
                    }
                    13 => {
                        // Enter — insert a newline, over the selection if there is one.
                        self.insert_char('\n');
                        self.request_redraw();
                    }
                    9 => {
                        // Tab is content in a document editor: indentation is text, not a focus
                        // gesture, so it enters the value instead of being dropped as a control
                        // character.
                        self.insert_char('\t');
                        self.request_redraw();
                    }
                    33 => {
                        // Page Up — one window of display rows toward the start, keeping the
                        // caret's column within its row.
                        let rows = self.display_rows();
                        let page = self.visible_line_count().saturating_sub(1).max(1);
                        let row = self.display_row_of(self.cursor_pos).saturating_sub(page);
                        if let Some(display) = rows.get(row).copied() {
                            let target = self.byte_index_at_column(
                                display.start,
                                display.end,
                                self.column_in_display_row(self.cursor_pos),
                            );
                            self.select_with_modifiers(target, mods);
                        }
                    }
                    34 => {
                        // Page Down — the mirror of Page Up.
                        let rows = self.display_rows();
                        let page = self.visible_line_count().saturating_sub(1).max(1);
                        let row = (self.display_row_of(self.cursor_pos) + page)
                            .min(rows.len().saturating_sub(1));
                        if let Some(display) = rows.get(row).copied() {
                            let target = self.byte_index_at_column(
                                display.start,
                                display.end,
                                self.column_in_display_row(self.cursor_pos),
                            );
                            self.select_with_modifiers(target, mods);
                        }
                    }
                    37 => {
                        // Left — one character back, extending under Shift. At the start the caret
                        // cannot move, but a plain press still owes the user a deselect.
                        if self.cursor_pos > 0 {
                            let target = self.previous_char_boundary();
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.request_redraw();
                        }
                    }
                    39 => {
                        // Right — one character forward; see the Left arm.
                        if self.cursor_pos < self.text.len() {
                            let target = self.next_char_boundary();
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.request_redraw();
                        }
                    }
                    38 => {
                        // Up — one *logical* line, keeping the column. The multi-line movement this
                        // control has and a single-line field cannot: without it the only way to
                        // reach another line was Home then Left, repeated.
                        let target = self.line_up();
                        self.select_with_modifiers(target, mods);
                    }
                    40 => {
                        // Down — see the Up arm.
                        let target = self.line_down();
                        self.select_with_modifiers(target, mods);
                    }
                    36 => {
                        // Home — the start of the *line*, or of the *document* with Primary held.
                        // Plain Home jumping to the document start is what a single-line field
                        // does; in a multi-line one it makes the key useless for reaching the line
                        // the caret is already on.
                        let target = if primary { 0 } else { self.line_start_of_caret() };
                        self.select_with_modifiers(target, mods);
                    }
                    35 => {
                        // End — the end of the line, or of the document with Primary held.
                        let target =
                            if primary { self.text.len() } else { self.line_end_of_caret() };
                        self.select_with_modifiers(target, mods);
                    }
                    _ => {
                        // Character input. A control chord is not text, and neither is an unmapped
                        // non-printable key.
                        //
                        // # Why the three modifiers are tested one at a time
                        //
                        // `Modifiers::contains` compares the *value* of the mask, so the
                        // combined `contains(CTRL | ALT | META)` form is true only when all
                        // three are held at once — a single-Control chord fails it and the
                        // character goes through. That is the defect this arm exists to stop, so
                        // the combined spelling would have fixed nothing.
                        if !mods.contains(crate::shortcut::Modifiers::CTRL)
                            && !mods.contains(crate::shortcut::Modifiers::ALT)
                            && !mods.contains(crate::shortcut::Modifiers::META)
                        {
                            // Only a **printable** key is content: `char::from_u32(0)` is NUL, and
                            // 9/10/13 are Tab/LF/CR — control codes the host owns, not text.
                            if let Some(ch) = char::from_u32(*key).filter(|c| !c.is_control()) {
                                if ch.is_ascii_graphic() || ch == ' ' {
                                    self.insert_char(ch);
                                    self.request_redraw();
                                }
                            }
                        }
                    }
                }
            }
            // ── Pointer selection ──
            //
            // The press places the caret where the user aimed and anchors a selection there; a
            // following drag extends it. Before this a press only focused the control, so clicking
            // into the middle of an existing document and typing appended instead of inserting —
            // and there was no pointer selection at all.
            Event::MousePress { pos, button, modifiers, .. }
                if *button == 1 && self.hit_test(*pos) =>
            {
                self.press_with_pointer(*pos, *modifiers);
            }
            Event::MouseMove { pos } => {
                if self.scrollbar_drag {
                    self.scrollbar_scroll_to(pos.y);
                    self.request_redraw();
                } else if self.dragging_selection {
                    let index = self.byte_index_at(*pos);
                    self.drag_to(index);
                }
            }
            // A double click selects the word under the pointer.
            Event::MouseDoubleClick { pos, button } if *button == 1 && self.hit_test(*pos) => {
                self.focused = true;
                let index = self.byte_index_at(*pos);
                let (start, end) = self.word_range_at(index);
                self.selection_anchor = Some(start);
                self.cursor_pos = end;
                self.dragging_selection = false;
                self.scroll_caret_into_view();
                self.notify_caret_change();
                self.base.request_redraw();
            }
            // A release anywhere ends the gesture, including outside the control: the anchor stays
            // where the press put it, so the selection the user made is kept.
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.scrollbar_drag = false;
                self.end_drag();
            }
            // ── Touch ──
            //
            // A finger takes the same route the pointer does: the contact places the caret and
            // anchors the selection, the lift ends the drag, and a withdrawn contact clears the
            // latches without leaving the gesture half-finished. Without these arms the control
            // was unreachable on a touch-only host. There is no separate `Tap` arm: a tap is the
            // press plus the release the two arms above already perform.
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } if self.hit_test(*pos) => {
                self.press_with_pointer(*pos, 0);
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } => {
                self.scrollbar_drag = false;
                self.end_drag();
            }
            #[cfg(feature = "touch")]
            event if crate::event::translator::is_touch_cancel(event) => {
                self.scrollbar_drag = false;
                self.end_drag();
            }
            _ => {}
        }
    }
}
