// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! [`LineEdit`]'s trait surface: `Widget`, `EventHandler`, the key router and drag/drop.

use super::model::*;

use crate::compat::ToString;
use crate::core::{Point, Size};
use crate::event::dnd::{DragPayload, DragSource, DropEffect, DropTarget};
use crate::event::{Event, EventHandler};
use crate::platform::EchoMode;

use crate::impl_widget_property_hooks;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::widget_trait::InputMethodQuery;
use crate::widget::{BaseWidget, Widget};

impl Widget for LineEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Reports the caret an IME candidate window should anchor to while this field is focused.
    ///
    /// A text field always has an insertion point, so the answer is always `Some`; the rectangle
    /// is derived from the same field band and font the paint path uses.
    fn input_method_query(&self) -> Option<InputMethodQuery> {
        Some(self.caret_query())
    }

    /// Turns a context-menu request into this field's signal.
    ///
    /// The field knows the point and the selection; the host knows the menu. Emitting the
    /// request (rather than opening a popup here) keeps the cut/copy/paste policy in one
    /// place and lets the same signal serve a right-click and a keyboard request.
    fn context_menu_event(&mut self, pos: Point) -> bool {
        self.context_menu_requested.emit(pos);
        true
    }

    /// A selected value is the field's drag payload.
    ///
    /// The payload uses the `text/plain` type so a drag can land in any other text control
    /// without an adapter. A masked field contributes its mask, exactly as copy does.
    fn drag_source_mut(&mut self) -> Option<&mut dyn DragSource> {
        Some(self)
    }

    /// The field accepts plain text dropped at the pointer.
    fn drop_target_mut(&mut self) -> Option<&mut dyn DropTarget> {
        Some(self)
    }

    fn size_hint(&self) -> Size {
        // The shared estimate plus the caret's own trailing room, rather than `len() * 8 + 10`.
        // The 10 px was making the same point — a text field leaves the caret somewhere to be —
        // but the multiplier was a byte count, so any non-ASCII value asked for three times the
        // width it would draw. The font is the **effective** one the draw path uses (D09-STYLE-02),
        // so a text scale or custom face moves the hint and the ink together.
        let font = effective_font(self.style());
        let text_w = estimate_text_width(self.text(), font, 1.0) + LINE_EDIT_CARET_ROOM;
        Size::new(text_w.max(LINE_EDIT_MIN_WIDTH), LINE_EDIT_HEIGHT)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    // The caret blink is the whole animation: without a trait-visible `tick` a host holding
    // `&mut dyn Widget` could not advance it, so the caret never blinked on screen.
    fn tick(&mut self, delta_ms: u32) -> bool {
        LineEdit::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.focused && !self.read_only
    }

    /// Reports the **interaction** state: `Disabled` when inert, else pressed / hovered / focused.
    ///
    /// # Why the refusal is no longer reported here
    ///
    /// This used to return `Error` while the field carried a refusal message. That made the
    /// refusal and the interaction compete for one value, and the interaction won whenever the
    /// pointer was over the field — so `"line_edit:error"` stopped applying exactly when the user
    /// moved the pointer onto the field to see what was wrong. The meaning moved to
    /// [`Self::semantic_state`], which is orthogonal and so holds at the same time as `Hover`.
    ///
    /// `Disabled` still outranks everything: an inert field cannot be interacted with, so the
    /// interaction channel is what describes it, and the meaning channel reports the refusal
    /// separately.
    fn widget_state(&self) -> crate::style::WidgetState {
        use crate::style::WidgetState;
        if !self.base.is_enabled() {
            return WidgetState::Disabled;
        }
        if self.base.is_pressed() {
            WidgetState::Pressed
        } else if self.base.is_hovered() {
            WidgetState::Hover
        } else if self.base.draws_focus_ring() {
            WidgetState::Focused
        } else {
            WidgetState::Normal
        }
    }

    /// Reports the field's **meaning**: `Error` while it carries a refusal or is over its limit.
    ///
    /// The other half of the split [`Widget::widget_state`] documents. The two are independent, so
    /// a hovered refusal keeps the hover fill **and** the error border — which is what the plan's
    /// criterion asks for and what a single-valued chain could not express.
    fn semantic_state(&self) -> crate::style::SemanticState {
        if !self.decorations.error.is_empty() || self.is_over_limit() {
            return crate::style::SemanticState::Error;
        }
        crate::style::SemanticState::None
    }
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
            "editing_finished" => {
                Some(EventSignalRef::unit("editing_finished", &self.editing_finished))
            }
            "return_pressed" => Some(EventSignalRef::unit("return_pressed", &self.return_pressed)),
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| CapabilityValue::String(format!("{v:?}")),
            )),
            "selection_changed" => {
                Some(EventSignalRef::unit("selection_changed", &self.selection_changed))
            }
            "cursor_position_changed" => Some(EventSignalRef::mapped(
                "cursor_position_changed",
                &self.cursor_position_changed,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            "text_edited" => Some(EventSignalRef::mapped("text_edited", &self.text_edited, |v| {
                CapabilityValue::String(v.clone())
            })),
            "action_triggered" => {
                Some(EventSignalRef::mapped("action_triggered", &self.action_triggered, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
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
            "copy_available" => {
                Some(EventSignalRef::mapped("copy_available", &self.copy_available, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            _ => None,
        }
    }
}

impl LineEdit {
    /// Routes a key event through the shared selection entry point instead of mutating
    /// `selection_start` directly.
    ///
    /// # Why the dispatch was pulled out of `handle_event`
    ///
    /// The four arrows and Home/End each carried their own copy of the shift test — six
    /// copies of one rule, each free to drift from the others. They now all call
    /// [`LineEdit::select_with_modifiers`] (the same entry point the pointer gesture uses),
    /// so what Shift means is written once (principle #101: one concept, one path).
    ///
    /// The helper is inherent rather than a trait method because `EventHandler` declares
    /// only `handle_event`; the trait impl below is a thin façade over it.
    ///
    /// Returns whether the key was consumed. A key this field does not handle leaves the
    /// event to the rest of the chain rather than being silently swallowed.
    pub(super) fn handle_key(&mut self, key: u32, modifiers: u32) -> bool {
        // The wire bitmask answers the primary question directly — a Desktop host also
        // accepts its Control key (see `event_carries_primary`) — while the `Modifiers`
        // value below serves the SHIFT checks and the selection extensions.
        let primary = crate::shortcut::event_carries_primary(modifiers);
        let modifiers = crate::shortcut::Modifiers::from_event_bits(modifiers);
        match key {
            8 => {
                // Backspace
                self.backspace();
            }
            46 => {
                // Delete
                self.delete();
            }
            13 => {
                // Enter/Return
                self.return_pressed.emit();
                self.editing_finished.emit();
            }
            27 => {
                // Escape
                self.editing_finished.emit();
            }
            37 => {
                // Left arrow. A plain press deselects and steps one character back; a
                // Shift press extends from the anchor rather than replacing it, so the
                // range grows instead of collapsing to a caret (see `select_with_modifiers`).
                // Primary (Ctrl/Cmd) steps by **word**, which is the convention every desktop
                // text field follows; the boundary helper only walks when it is held.
                let by_word = primary;
                if self.cursor_position > 0 {
                    let target = if by_word {
                        self.previous_word_boundary(self.cursor_position)
                    } else {
                        self.previous_char_boundary()
                    };
                    self.select_with_modifiers(target, modifiers);
                } else if !modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
                    // Already at the start, and no Shift: the key cannot move the caret, but it
                    // still owes the user the deselect every other plain press performs. It used
                    // to be dropped entirely, which left a stale range alive after an arrow press
                    // that plainly meant "just move". A Shift press here has nothing to extend to.
                    self.clear_selection();
                    self.base.request_redraw();
                }
            }
            39 => {
                // Right arrow. Same contract as Left, stepping forward.
                let by_word = primary;
                if self.cursor_position < self.text.len() {
                    let target = if by_word {
                        self.next_word_boundary(self.cursor_position)
                    } else {
                        self.next_char_boundary()
                    };
                    self.select_with_modifiers(target, modifiers);
                } else if !modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
                    // Already at the end: see the Left arm — the deselect still applies.
                    self.clear_selection();
                    self.base.request_redraw();
                }
            }
            36 => {
                // Home
                self.select_with_modifiers(0, modifiers);
            }
            35 => {
                // End
                self.select_with_modifiers(self.text.len(), modifiers);
            }
            65 if primary => {
                // Primary+A: Select all
                self.select_all();
            }
            86 if primary => {
                // Primary+V: Paste from the platform clipboard.
                #[cfg(not(alloc_frugal))]
                {
                    let pasted = crate::get_clipboard_text();
                    if !pasted.is_empty() {
                        self.insert_str(&pasted);
                    }
                }
                #[cfg(alloc_frugal)]
                {
                    // Clipboard integration is unavailable in the mini profile.
                    self.base.redraw_requested.emit();
                }
            }
            67 if primary => {
                // Primary+C: Copy selection to the platform clipboard.
                #[cfg(not(alloc_frugal))]
                {
                    self.copy_selection_to_clipboard();
                }
                #[cfg(alloc_frugal)]
                {
                    // Clipboard integration is unavailable in the mini profile.
                    self.base.redraw_requested.emit();
                }
            }
            88 if primary => {
                // Primary+X: Copy selection, then delete it.
                //
                // The two halves have different conditions on purpose. What reaches the
                // clipboard is `clipboard_selection`, which a masked field limits to its mask
                // (and `NoEcho` to nothing); the *deletion* is the edit the user asked for and
                // happens whenever there is a selection, so a cut in a password field still
                // removes what the user cut.
                #[cfg(not(alloc_frugal))]
                {
                    let had_selection = !self.selected_text().is_empty();
                    if let Some(text) = self.clipboard_selection() {
                        crate::set_clipboard_text(&text);
                    }
                    if had_selection {
                        self.backspace(); // removes the selection
                    }
                }
                #[cfg(alloc_frugal)]
                {
                    // Clipboard integration is unavailable in the mini profile.
                    self.base.redraw_requested.emit();
                }
            }
            90 if primary && modifiers.contains(crate::shortcut::Modifiers::SHIFT) => {
                // Primary+Shift+Z is the other conventional redo chord. It used to fall into the
                // plain undo arm below, so the field undid the edit the user meant to redo; the
                // other editable controls in this family bind it (I-01).
                let _ = self.redo();
            }
            90 if primary => {
                let _ = self.undo();
            }
            89 if primary => {
                let _ = self.redo();
            }
            _ => {
                // Character input. A control chord must not be swallowed as text, nor may an
                // unmapped **non-printable** key be: both used to be typed as their bare
                // character, so `Ctrl+B` inserted `b` and `Ctrl+Z` inserted `z`.
                //
                // # Why the three modifiers are tested one at a time
                //
                // Combining them with `|` and asking `contains` is not the predicate it reads
                // as: `Modifiers::contains` compares the *value* of the mask, so
                // `contains(CTRL | ALT | META)` is true only when **all three** are held at once.
                // A `Ctrl+B` carries CTRL alone and therefore failed the combined test, which is
                // exactly the defect this arm exists to stop. Three separate `contains` calls say
                // what is meant: none of the three is held.
                let chord_held = modifiers.contains(crate::shortcut::Modifiers::CTRL)
                    || modifiers.contains(crate::shortcut::Modifiers::ALT)
                    || modifiers.contains(crate::shortcut::Modifiers::META);
                if !chord_held {
                    // Only a **printable** key is content. `char::from_u32(0)` is `\0`, and 9, 10
                    // and 13 are Tab/Enter/CR — none of them is something a user typed into a
                    // text field, and all of them are control codes the host owns.
                    if let Some(ch) = char::from_u32(key).filter(|c| !c.is_control()) {
                        if ch.is_ascii_graphic() || ch == ' ' {
                            self.insert_str(&ch.to_string());
                        }
                    }
                }
            }
        }
        true
    }
}

impl LineEdit {
    /// Tells the host that the value changed during this event.
    ///
    /// The queue is only touched for a **mounted** field: a value callback is registered through
    /// the widget registry, so an unmounted `LineEdit` (the shape most unit tests use) has no
    /// reader, and injecting there would put an event into a process-wide queue that no frame will
    /// drain. The trigger carries no text — [`crate::app::dispatch_trigger`] reads the live value —
    /// so a later edit cannot make an earlier notification describe the wrong string.
    ///
    /// `revision` is the [`Self::text_revision`] read before the gesture; a gesture that did not
    /// move the value (every arrow, Home/End, or a refused insert) announces nothing (I-P01).
    #[cfg(not(alloc_frugal))]
    pub(super) fn announce_value_changed_if_revision_moved(&self, revision: u64) {
        if self.text_revision == revision {
            return;
        }
        // A widget's own `base().id()` is not the id the registry mounted it under, and the
        // trigger queue and callback table are keyed by the **registry** id. Resolving the
        // translation here is what makes the notification address the control the host holds;
        // injecting with the own id would queue an event no callback could ever match.
        let Some(id) = crate::widget::runtime::registry_id_of(self.base.id()) else {
            return;
        };
        if !crate::widget::runtime::is_mounted(id) {
            return;
        }
        let _ = crate::inject_widget_trigger_event(
            id,
            crate::platform::WidgetTriggerKind::ValueChanged,
        );
    }

    /// The allocation-frugal build has no trigger queue and no host callbacks.
    #[cfg(alloc_frugal)]
    pub(super) fn announce_value_changed_if_revision_moved(&self, _revision: u64) {}
}

impl EventHandler for LineEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if self.read_only {
            // Read-only fields still allow copying the current selection. The chord is the same
            // primary convention the editable path uses, so Command+C works on macOS too; the
            // bare `2` this used to test was the Control bit only (I-013).
            #[cfg(not(alloc_frugal))]
            if let Event::KeyPress { key: 67, modifiers } = event {
                if crate::shortcut::event_carries_primary(*modifiers) {
                    self.copy_selection_to_clipboard();
                }
            }
            return;
        }
        match event {
            Event::KeyPress { key, modifiers } => {
                // A user edit has to reach `on_value_changed` without the host polling the text.
                //
                // The trigger is queued only when the value actually moved: every navigation key
                // (`Left`, `Home`, a shortcut chord) also arrives here, and announcing those would
                // make a form validate on a keystroke that changed nothing. `set_text` is
                // deliberately not the injection point because a host calling it already knows the
                // value it just wrote.
                let revision = self.text_revision;
                self.handle_key(*key, *modifiers);
                self.announce_value_changed_if_revision_moved(revision);
                self.emit_text_edited_if_revision_moved(revision);
            }
            // Platform-committed text (D09-INPUT-01).
            //
            // The desktop backends deliver a printable character or an IME commit as
            // `Event::TextInput`/`Event::ImeCommit`, not as a `KeyPress`, so a handler that only
            // matches `KeyPress` never receives what the user typed. Routing both variants into
            // `insert_text` — the same entry point the `KeyPress` printable path calls — keeps the
            // field's focus, read-only, selection-replacement, `max_length`, undo and
            // `text_changed` contract in one place rather than duplicating it here. The
            // `read_only`/`is_enabled` early returns above already gate this branch.
            Event::TextInput { text } => {
                let revision = self.text_revision;
                self.insert_str(text);
                self.announce_value_changed_if_revision_moved(revision);
                self.emit_text_edited_if_revision_moved(revision);
            }
            // A commit ends the composition as well as inserting its text. Clearing the preedit
            // first is what keeps the underlined preview from surviving the string it previewed:
            // an input method may deliver the commit before its `preedit_end`, and the field would
            // otherwise paint both the committed text and a stale composition.
            Event::ImeCommit { text } => {
                let revision = self.text_revision;
                self.composition = None;
                self.insert_str(text);
                self.announce_value_changed_if_revision_moved(revision);
                self.emit_text_edited_if_revision_moved(revision);
            }
            // The uncommitted composition belongs to the field while the user is building it: the
            // value does not change, but the inline preedit at the caret does. Without this arm an
            // input method's preview reached no text model at all, because `set_composition` had
            // no event producer inside the widget layer.
            Event::ImePreedit { text, .. } => {
                self.set_composition(text);
            }
            Event::FocusLost => {
                self.set_focused(false);
                self.editing_finished.emit();
            }
            Event::FocusGained { .. } => {
                self.set_focused(true);
            }
            // A press focuses the field only when it lands on the **painted band**, not on
            // the control's rectangle. The two used to be the same box, so the hit test
            // silently agreed with the ink by accident; once the field became a centred
            // 48 px band in a 120 px cell, testing `geometry()` would let a user focus the
            // field by clicking 60 px below it — on the window background, nowhere near
            // any ink. The test is against `field_rect()` for exactly that reason.
            Event::MousePress { pos, button, modifiers, .. }
                if *button == 1 && self.field_rect().contains_point(*pos) =>
            {
                self.press_with_pointer(*pos, *modifiers);
            }
            // A double click selects the word under the pointer; a third click in the same streak
            // selects the whole value, which is the convention for single-line fields.
            Event::MouseDoubleClick { pos, button }
                if *button == 1 && self.field_rect().contains_point(*pos) =>
            {
                let index = self.byte_index_at_x(pos.x);
                if self.press_streak >= 3 {
                    self.select_all();
                } else {
                    let before_cursor = self.cursor_position;
                    let before_selection = self.selection_start;
                    let (start, end) = self.word_range_at(index);
                    self.selection_start = Some(start);
                    self.cursor_position = end;
                    self.dragging_selection = false;
                    self.notify_caret_change(before_cursor, before_selection);
                    self.base.request_redraw();
                }
            }
            // A drag extends the selection the press began, and only then — see `drag_to`.
            Event::MouseMove { pos } => {
                if self.dragging_selection {
                    let index = self.byte_index_at_x(pos.x);
                    self.drag_to(index);
                }
            }
            // A release anywhere ends the gesture, including outside the field: the anchor
            // stays where the press put it so the selection the user made is kept.
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.end_drag();
            }
            // ── Touch ──
            //
            // A finger takes the same route the pointer does: the contact places the caret and
            // anchors the selection, the lift ends the gesture, and a withdrawn contact clears
            // the same latch. These arms live here rather than in each factory because the
            // password and email derivations are this control with a different echo mode, so
            // they inherit touch support. No separate `Tap` arm: a tap is the press plus the
            // release performed here.
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } if self.field_rect().contains_point(*pos) => {
                self.press_with_pointer(*pos, 0);
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } => {
                self.end_drag();
            }
            #[cfg(feature = "touch")]
            event if crate::event::translator::is_touch_cancel(event) => {
                self.end_drag();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
/// A selected value is the field's drag payload.
///
/// The payload uses the `text/plain` type so it can land in any other text control without an
/// adapter, and a masked field contributes the mask rather than the plaintext — the same rule
/// its copy path follows.
impl DragSource for LineEdit {
    fn drag_payload(&self, pos: Point) -> Option<DragPayload> {
        // The switch is consulted before anything is measured, so a field that did not opt in
        // pays nothing for a gesture it will refuse.
        if !self.drag_enabled || !self.field_rect().contains_point(pos) {
            return None;
        }
        let selected = self.selected_text();
        if selected.is_empty() {
            return None;
        }
        let carried = match self.echo_mode {
            EchoMode::Normal => selected.clone(),
            EchoMode::Password => Self::mask_text(&selected),
            EchoMode::NoEcho => return None,
        };
        Some(DragPayload::new("text/plain", carried.clone()).with_label(carried).with_origin(pos))
    }

    /// A `Move` drop removes the source selection; every other effect keeps it.
    fn on_drag_finished(&mut self, effect: DropEffect) {
        if !effect.consumes_source() || self.read_only {
            return;
        }
        if !self.selected_text().is_empty() {
            let revision = self.text_revision;
            self.backspace();
            self.modified = true;
            self.emit_text_edited_if_revision_moved(revision);
        }
    }
}

/// The field accepts plain text and inserts it at the drop point.
impl DropTarget for LineEdit {
    fn can_accept(&self, payload: &DragPayload) -> bool {
        !self.read_only && payload.is_type("text/plain")
    }

    fn on_drop(&mut self, payload: &DragPayload, pos: Point) -> DropEffect {
        if !self.can_accept(payload) || !self.field_rect().contains_point(pos) {
            return DropEffect::None;
        }
        let text =
            if payload.label.is_empty() { payload.item_id.clone() } else { payload.label.clone() };
        if text.is_empty() {
            return DropEffect::None;
        }
        self.press_at(self.byte_index_at_x(pos.x), crate::shortcut::Modifiers::empty());
        self.end_drag();
        let revision = self.text_revision;
        self.insert_str(&text);
        self.modified = true;
        self.emit_text_edited_if_revision_moved(revision);
        self.base.request_redraw();
        DropEffect::Copy
    }
}
