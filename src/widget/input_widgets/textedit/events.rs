// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! [`TextEdit`]'s trait surface: `Widget`, `DragSource`, `WidgetProperties` and the event router.

use super::model::*;

use crate::core::{Point, Size};
use crate::event::dnd::{DragPayload, DragSource, DropEffect};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::widget::capability::coercion::{
    expect_horizontal_alignment, horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::widget_trait::InputMethodQuery;
use crate::widget::{BaseWidget, Widget};

// Implement Widget trait
impl Widget for TextEdit {
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
        Size::new(dimensions::TEXT_EDIT_DEFAULT_WIDTH, dimensions::TEXT_EDIT_DEFAULT_HEIGHT)
    }

    /// Advances the caret blink and reports whether another frame is owed (I-119).
    fn tick(&mut self, delta_ms: u32) -> bool {
        TextEdit::tick(self, delta_ms)
    }

    /// A focused editor keeps a blinking caret, so the host must keep scheduling frames (I-119).
    fn is_animating(&self) -> bool {
        self.focused
    }

    /// Reports the caret rectangle an IME candidate window anchors to (I-120).
    fn input_method_query(&self) -> Option<InputMethodQuery> {
        Some(self.caret_query())
    }

    /// Turns a context-menu request into this editor's signal; the host supplies the menu.
    fn context_menu_event(&mut self, pos: Point) -> bool {
        self.context_menu_requested.emit(pos);
        true
    }

    /// Opts the editor into the runtime's drag coordinator as a source.
    fn drag_source_mut(&mut self) -> Option<&mut dyn DragSource> {
        Some(self)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
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

/// A selected run of text is the editor's drag payload.
///
/// Mirrors [`TextArea`](super::textarea::TextArea): the item id and label both hold the text, so
/// a `text/plain` target can read either field without a rule about which is authoritative.
impl DragSource for TextEdit {
    fn drag_payload(&self, pos: Point) -> Option<DragPayload> {
        if !self.geometry().contains_point(pos) {
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
        if self.backspace() {
            self.base.request_redraw();
        }
    }
}

/// `TextEdit`'s property contract.
///
/// `WidgetKind::TextEdit` is the kind the capability layer pairs with the
/// [`TerminalView`](crate::widget::TerminalView) control
/// (`capability::properties::terminal_view_capability`), so the old centralised
/// `TextEdit` arms were answered by `TerminalView`, not by this widget — despite
/// `TEXT_EDIT_PROPERTIES` existing, its names are all marked non-readable and
/// non-writable, so the multi-line editor below never served a property. The
/// contract for the kind lives beside `TerminalView`; this widget publishes none
/// of its own rather than claiming another control's.
impl WidgetProperties for TextEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text.clone())),
            "placeholder_text" => Ok(CapabilityValue::String(self.placeholder_text.clone())),
            "max_length" => match self.max_length {
                // An unset limit is `Null`, not a numeric sentinel. `LineEdit` answers
                // the same question the same way, and the schema declares this property
                // `UInt` — so returning `u64::MAX` for "unset" made the value
                // un-writable: reading a `textedit` with no limit and writing the value
                // back installed a nonsensical cap (or failed outright on a 32-bit
                // target, where `usize::try_from(u64::MAX)` is out of range). A property
                // whose read cannot be written back is a broken round trip.
                Some(limit) => Ok(CapabilityValue::UInt(limit as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "read_only" => Ok(CapabilityValue::Bool(self.read_only)),
            "line_wrap" => Ok(CapabilityValue::Bool(self.line_wrap)),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        // These five are real, working accessors on this control, so the
        // property route must reach them. Forwarding everything to
        // `base_property_set` meant `rw_widget_property_set(id, "text", ..)`
        // answered `UnknownProperty` even though `set_text` worked.
        match name {
            "text" => match value {
                CapabilityValue::String(text) => {
                    self.set_text(text);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "placeholder_text" => match value {
                CapabilityValue::String(text) => {
                    self.set_placeholder_text(text);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "max_length" => match value {
                // `Null` is the getter's "no limit" answer (and the schema default), so
                // the setter must accept it — `LineEdit` already does, and a property
                // whose read cannot be written back is a broken round trip.
                CapabilityValue::Null => {
                    self.set_max_length(None);
                    return Ok(());
                }
                CapabilityValue::UInt(limit) => {
                    let limit =
                        usize::try_from(limit).map_err(|_| CapabilityAccessError::OutOfRange)?;
                    self.set_max_length(Some(limit));
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "read_only" => match value {
                CapabilityValue::Bool(flag) => {
                    self.set_read_only(flag);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "line_wrap" => match value {
                CapabilityValue::Bool(flag) => {
                    self.set_line_wrap(flag);
                    return Ok(());
                }
                _ => return Err(CapabilityAccessError::TypeMismatch),
            },
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                return Ok(());
            }
            _ => {}
        }
        base_property_set(self, name, value)
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Must list every name `TEXT_EDIT_PROPERTIES` declares, because `get` /
        // `set` below answer all five. Publishing only the shared four while the
        // schema promised these names is the mismatch
        // `schema_and_contract_publish_the_same_names` exists to catch.
        property_names_of![
            "text",
            "placeholder_text",
            "max_length",
            "read_only",
            "line_wrap",
            "alignment",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `text_edit` publishes.
    ///
    /// Every name in the set assigns state — the text, the placeholder, the length
    /// limit, the read-only flag, the wrap mode — and each needs a payload, so the whole
    /// set is answered through the property route. The names are placed in
    /// `TEXT_EDIT_PROPERTIES` rather than served by this widget (see the module docs on
    /// why), but the capability still resolves `text_edit` to `TextEdit`, so the refusal
    /// has to live here: returning `UnknownCommand` would make `invoke_command` report
    /// `UnsupportedOnWidget` for names the control does publish.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text"
            | "set_placeholder_text"
            | "set_max_length"
            | "set_read_only"
            | "set_line_wrap" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
impl EventHandler for TextEdit {
    fn handle_event(&mut self, event: &Event) {
        // A queued find/replace intent is a shared write, so it is applied before the event is
        // routed and the frame that follows the strip click already reflects it (COMP-A8).
        #[cfg(full_widgets)]
        self.poll_find_replace_bar();
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::FocusGained { .. } => {
                self.focused = true;
                // I-119: the caret only blinks while the editor owns focus.
                self.cursor_blink.start();
                self.base.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.cursor_blink.stop();
                self.base.request_redraw();
            }
            // A press places the caret and starts a selection. It used to do nothing at all, so the
            // only way to put the caret anywhere was to delete to that point.
            Event::MousePress { pos, button, modifiers, .. } if *button == 1 => {
                self.press_with_pointer(*pos, *modifiers);
            }
            Event::MouseMove { pos } => {
                if self.scrollbar_drag {
                    self.scrollbar_scroll_to(pos.y);
                    self.base.request_redraw();
                } else if self.dragging_selection {
                    let offset = self.offset_at_point(*pos);
                    self.drag_to(offset);
                }
            }
            // A double click selects the word under the pointer.
            Event::MouseDoubleClick { pos, button } if *button == 1 => {
                self.focused = true;
                let offset = self.offset_at_point(*pos);
                let (start, end) = self.word_range_at(offset);
                self.selection_anchor = Some(start);
                self.cursor = end;
                self.dragging_selection = false;
                self.notify_caret_change();
                self.base.request_redraw();
            }
            // A release anywhere ends the gesture: the anchor stays where the press put it, so the
            // selection the user made is kept.
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.scrollbar_drag = false;
                self.end_drag();
            }
            // ── Touch ──
            //
            // A finger takes the same route the pointer does: the contact places the caret and
            // anchors the selection, the lift ends the gesture, and a withdrawn contact clears the
            // latches without leaving it half-finished. Without these arms the control was
            // unreachable on a touch-only host. No separate `Tap` arm: a tap is the press plus the
            // release the arms here already perform.
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
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
            // Platform-committed text (D09-INPUT-01), routed into the same `insert_str` the
            // `KeyPress` printable arm below calls. This is where a printable character or an IME
            // commit actually arrives from the desktop backends, so without this branch typing did
            // not reach the value. `read_only` is checked here for the same reason it is checked in
            // the key path — a read-only editor must not accept committed text either.
            Event::TextInput { text } | Event::ImeCommit { text } => {
                if !self.focused || self.read_only {
                    return;
                }
                self.insert_str(text);
            }
            // Wheel: positive `delta.y` moves the content down, so the viewport moves toward the
            // start. The shared helper turns wheel notches into lines at the desktop default of
            // three lines per notch.
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
                // I-122: the horizontal component is consumed too, so an unwrapped long line is
                // reachable by a horizontal wheel or trackpad gesture.
                let moved =
                    self.scroll_rows(-scroll.lines_y) | self.scroll_columns(scroll.columns_x);
                if moved {
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, modifiers } => {
                // An unfocused editor owns no keys — see the `focused` field for what that prevents.
                if !self.focused {
                    return;
                }
                // The event carries the framework's wire bitmask; translate it once, here, so nothing
                // below re-derives a bit.
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = mods.contains(crate::shortcut::Modifiers::SHIFT);
                let primary = crate::shortcut::event_carries_primary(*modifiers);

                // I-121: selection and copy stay available on a read-only editor, but undo/redo
                // must not be: the history path is still a mutation.
                if primary && *key == 65 {
                    // Primary+A: select the whole document.
                    self.select_all();
                    return;
                }
                if primary && *key == 67 {
                    // Primary+C: copy the selection. Allowed while read-only, which is the one
                    // edit gesture a read-only editor still owes the user.
                    let _ = self.copy_selection_to_clipboard();
                    return;
                }
                if self.read_only {
                    return;
                }
                if primary && *key == 90 {
                    let _ = self.undo();
                    return;
                }
                if primary && *key == 89 {
                    let _ = self.redo();
                    return;
                }
                if primary && *key == 88 {
                    // Primary+X: copy, then remove the selection.
                    let _ = self.copy_selection_to_clipboard();
                    self.backspace();
                    return;
                }
                if primary && *key == 86 {
                    // Primary+V: paste at the caret, replacing any selection.
                    self.paste_from_clipboard();
                    return;
                }

                match *key {
                    8 => {
                        self.backspace();
                    }
                    46 => {
                        // Delete — the character *after* the caret. Unhandled before, so it fell
                        // through to the printable arm where key 46 is `.`.
                        self.delete_forward();
                    }
                    13 => {
                        // Enter — a newline at the caret, replacing the selection like any insert.
                        self.insert_str("\n");
                    }
                    37 => {
                        // Left — one character back. At the start the caret cannot move, but a plain
                        // press still owes the user a deselect.
                        if self.cursor > 0 {
                            let target = self.previous_char_boundary(self.cursor);
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.base.request_redraw();
                        }
                    }
                    39 => {
                        // Right — one character forward; see the Left arm.
                        if self.cursor < self.text.len() {
                            let target = self.next_char_boundary(self.cursor);
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.base.request_redraw();
                        }
                    }
                    // Up/Down move by **laid-out row**, not by logical line: this control wraps, so
                    // the row above is what "up" means to a user looking at the screen.
                    //
                    // The wrap table is built **once** here and passed to both answers. It used to be
                    // rebuilt by `row_of_offset` and again by `offset_in_row` (which itself asked for
                    // `row_of_offset`), so one press laid the whole document out three times.
                    38 => {
                        let rows = self.layout_rows();
                        let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                        if row > 0 {
                            let target = self.offset_in_row_with(&rows, rows[row - 1], self.cursor);
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.base.request_redraw();
                        }
                    }
                    40 => {
                        let rows = self.layout_rows();
                        let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                        if row + 1 < rows.len() {
                            let target = self.offset_in_row_with(&rows, rows[row + 1], self.cursor);
                            self.select_with_modifiers(target, mods);
                        } else if !shift {
                            self.clear_selection();
                            self.base.request_redraw();
                        }
                    }
                    36 => {
                        // Home — the start of the *row*, or of the document with Primary held.
                        // Plain Home reaching the document start would make it useless for
                        // returning to the line being edited.
                        let target = if primary {
                            0
                        } else {
                            let rows = self.layout_rows();
                            let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                            rows.get(row).map_or(0, |row| row.start)
                        };
                        self.select_with_modifiers(target, mods);
                    }
                    33 => {
                        // Page Up — one window of rows toward the start, keeping the column.
                        let rows = self.layout_rows();
                        let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                        let page = self.visible_row_count().saturating_sub(1).max(1);
                        let target_row = row.saturating_sub(page);
                        let target = rows
                            .get(target_row)
                            .map_or(0, |span| self.offset_in_row_with(&rows, *span, self.cursor));
                        self.select_with_modifiers(target, mods);
                    }
                    34 => {
                        // Page Down — the mirror of Page Up.
                        let rows = self.layout_rows();
                        let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                        let page = self.visible_row_count().saturating_sub(1).max(1);
                        let target_row = (row + page).min(rows.len().saturating_sub(1));
                        let target = rows.get(target_row).map_or(self.text.len(), |span| {
                            self.offset_in_row_with(&rows, *span, self.cursor)
                        });
                        self.select_with_modifiers(target, mods);
                    }
                    35 => {
                        // End — the end of the row, or of the document with Primary held.
                        let target = if primary {
                            self.text.len()
                        } else {
                            let rows = self.layout_rows();
                            let row = Self::row_of_offset_in(&rows, &self.text, self.cursor);
                            rows.get(row).map_or(self.text.len(), |row| {
                                // Exclude the row's own newline, so End is the end of the text the
                                // user sees rather than the start of the next row.
                                let end = row.end.min(self.text.len());
                                if self.text[..end].ends_with('\n') {
                                    end - 1
                                } else {
                                    end
                                }
                            })
                        };
                        self.select_with_modifiers(target, mods);
                    }
                    _ => {
                        // Printable character. A control chord is not text: the catch-all used to
                        // accept every key in the printable range regardless of the modifiers beside
                        // it, so `Primary+B` was typed as a literal `b`.
                        //
                        // The three modifiers are tested one at a time on purpose:
                        // `Modifiers::contains` compares the *value* of the mask, so the combined
                        // `contains(CTRL | ALT | META)` form is true only when all three are held.
                        let chord = mods.contains(crate::shortcut::Modifiers::CTRL)
                            || mods.contains(crate::shortcut::Modifiers::ALT)
                            || mods.contains(crate::shortcut::Modifiers::META);
                        if !chord {
                            if let Some(ch) = char::from_u32(*key).filter(|c| !c.is_control()) {
                                if ch.is_ascii_graphic() || ch == ' ' || ch == '\t' {
                                    self.insert_str(&ch.to_string());
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
