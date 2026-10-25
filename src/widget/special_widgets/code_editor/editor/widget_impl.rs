// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `Widget` and property contracts.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

// ─────────────────────────────────────────────────────────────────────────────
// Widget integration
// ─────────────────────────────────────────────────────────────────────────────

impl Widget for CodeEditor {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    /// The pointer is an I-beam over the **text**, and an arrow over the chrome.
    ///
    /// Zed does the same: the buffer is a text surface (you place a caret and select
    /// in it), while the gutter, the minimap and the scrollbar are controls that
    /// respond to clicks but are not text. One shape for the whole widget announced
    /// "text" over a scrollbar, which reads as a broken hit target.
    fn cursor_icon_at(&self, point: crate::core::Point) -> crate::platform::CursorIcon {
        use crate::platform::CursorIcon;
        // A popup is a list of clickable rows.
        if self.context_menu.visible
            && self.context_menu_rect().is_some_and(|rect| rect.contains_point(point))
        {
            return CursorIcon::Pointer;
        }
        if self.completion.visible && self.completion_rect().contains_point(point) {
            return CursorIcon::Pointer;
        }
        // Tabs are clickable chrome; a text cursor over them would claim the strip is
        // editable.
        if self.tab_hits.borrow().iter().any(|slot| slot.rect.contains_point(point)) {
            return CursorIcon::Pointer;
        }
        let rect = self.geometry();
        let text_top = rect.y + self.text_origin_y();
        let text_bottom = rect.y + rect.height as i32 - self.status_bar_height();
        let minimap_left = rect.x + rect.width as i32 - self.minimap_width();
        // The minimap is a navigation control: an arrow (and it has its own click
        // target, see `hit_minimap`).
        if point.x >= minimap_left && point.y >= text_top && point.y < text_bottom {
            return CursorIcon::Default;
        }
        if self.coordinate_map().text_area_contains(point) {
            CursorIcon::Text
        } else {
            CursorIcon::Default
        }
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
        // The palette was resolved once at build time, so switching between light
        // and dark left every token, selection and gutter colour on the old
        // appearance. Rebuild it here, where the appearance change actually
        // arrives, and invalidate the row measurements shaped with the old one.
        let palette = SyntaxPalette::for_active_appearance();
        if palette != self.palette {
            self.palette = palette;
            self.layout_lexer_generation = self.layout_lexer_generation.wrapping_add(1);
            self.row_layout_key.set((
                u64::MAX,
                u64::MAX,
                0,
                0,
                0,
                0,
                LanguageId::PlainText,
                u64::MAX,
            ));
            self.palette_changed.emit();
            self.base.request_redraw();
        }
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// `CodeEditor` paints itself, so it can be mounted into a native window.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    fn size_hint(&self) -> Size {
        Size::new(640, 400)
    }

    fn accessible_name(&self) -> String {
        let tooltip = self.tooltip().trim();
        if tooltip.is_empty() {
            format!("Code editor ({})", self.language_name())
        } else {
            tooltip.to_string()
        }
    }

    fn accessible_description(&self) -> String {
        format!(
            "CodeEditor {} lines {}, caret {}:{}",
            self.language_name(),
            self.line_count(),
            self.cursor.head.line + 1,
            self.cursor.head.column + 1
        )
    }

    /// The editor announces its selection, or the line the caret is on.
    ///
    /// A generic value lookup would find nothing useful here: the editor publishes no `value`
    /// property (its buffer is far too large to read on every announcement), so the default would
    /// announce empty and a screen-reader user would hear the editor's name with no sense of where
    /// they are. The line under the caret is the fact a user is actually navigating by, and a
    /// selection supersedes it because that is what the next command will act on.
    fn accessible_value(&self) -> String {
        let (start, end) = self.cursor.bounds();
        if start != end {
            return self.selected_text().unwrap_or_default();
        }
        self.line_text(self.cursor.head.line).unwrap_or_default()
    }

    impl_widget_property_hooks!();

    // The caret blink is driven through the trait so the animation bus reaches it -- an
    // editor whose caret never blinks reads as frozen.
    fn tick(&mut self, delta_ms: u32) -> bool {
        CodeEditor::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        (self.is_editable() && self.focused)
            || self.smooth_scroll_target.is_some()
            || (self.config.fade_popups
                && (self.completion.visible || self.context_menu.visible)
                && self.popup_reveal < 1.0)
            || (self.config.animate_folds && self.fold_reveal < 1.0)
            || (self.config.animate_brackets && self.bracket_flash > 0.0)
    }
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
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "cursor_moved" => {
                Some(EventSignalRef::mapped("cursor_moved", &self.cursor_moved, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ])
                }))
            }
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    match v {
                        Some(value) => CapabilityValue::String(format!("{value:?}")),
                        None => CapabilityValue::Null,
                    }
                }))
            }
            "tab_changed" => Some(EventSignalRef::mapped("tab_changed", &self.tab_changed, |v| {
                CapabilityValue::UInt(*v as u64)
            })),
            "fold_changed" => {
                Some(EventSignalRef::mapped("fold_changed", &self.fold_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "search_changed" => {
                Some(EventSignalRef::mapped("search_changed", &self.search_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "completion_changed" => {
                Some(EventSignalRef::mapped("completion_changed", &self.completion_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "search_result" => {
                Some(EventSignalRef::mapped("search_result", &self.search_result, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                        CapabilityValue::UInt(v.2 as u64),
                        CapabilityValue::UInt(v.3 as u64),
                    ])
                }))
            }
            "zoom_changed" => {
                Some(EventSignalRef::mapped("zoom_changed", &self.zoom_changed, |v| {
                    CapabilityValue::Float(*v as f64)
                }))
            }
            "breakpoint_toggled" => {
                Some(EventSignalRef::mapped("breakpoint_toggled", &self.breakpoint_toggled, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::Bool(v.1),
                    ])
                }))
            }
            "marker_activated" => {
                Some(EventSignalRef::mapped("marker_activated", &self.marker_activated, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "palette_changed" => {
                Some(EventSignalRef::unit("palette_changed", &self.palette_changed))
            }
            _ => None,
        }
    }
}

/// `CodeEditor`'s property contract, published under the `RichEdit` kind.
///
/// The capability layer makes `WidgetKind::RichEdit` the kind of this control
/// (`code_editor_capability`), and this impl reproduces exactly what the old
/// `read_input_props` / `write_input_props` arms answered for that kind. The
/// `RichEdit` widget is a different control that shares the kind; it keeps its
/// own file-local contract, and the downcast decides which one answers.
impl WidgetProperties for CodeEditor {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "line_count" => Ok(CapabilityValue::UInt(self.line_count() as u64)),
            "cursor_line" => Ok(CapabilityValue::UInt(self.cursor().0 as u64)),
            "cursor_column" => Ok(CapabilityValue::UInt(self.cursor().1 as u64)),
            "marker_count" => Ok(CapabilityValue::UInt(self.markers().len() as u64)),
            "undo_stack_limit" => Ok(CapabilityValue::UInt(self.config.undo_stack_limit as u64)),
            "undo_depth" => Ok(CapabilityValue::UInt(self.undo_depth() as u64)),
            "goto_bar_visible" => Ok(CapabilityValue::Bool(self.goto_bar_visible())),
            "breakpoint_gutter" => Ok(CapabilityValue::Bool(self.is_breakpoint_gutter())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            // Position, size and diagnostic count are derived from the buffer and
            // the caret.
            "line_count" | "cursor_line" | "cursor_column" | "marker_count" | "undo_depth" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            "undo_stack_limit" => {
                let limit = crate::widget::capability::coercion::expect_usize(value)?;
                self.set_undo_limit(limit);
                Ok(())
            }
            "goto_bar_visible" => {
                let visible = crate::widget::capability::coercion::expect_bool(value)?;
                self.set_goto_bar_visible(visible);
                Ok(())
            }
            "breakpoint_gutter" => {
                let visible = crate::widget::capability::coercion::expect_bool(value)?;
                self.set_breakpoint_gutter(visible);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // `CODE_EDITOR_PROPERTIES` declares `marker_count` readable, so the
        // contract must answer it: a name in the schema that no contract serves is
        // a promise the caller cannot keep, and the bidirectional schema test
        // rejects exactly that.
        property_names_of![
            "text",
            "line_count",
            "cursor_line",
            "cursor_column",
            "marker_count",
            "undo_stack_limit",
            "undo_depth",
            "goto_bar_visible",
            "breakpoint_gutter",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `code_editor` publishes.
    ///
    /// `undo` is the genuine zero-argument action here. It reports `false` when the
    /// undo stack is empty or the editor is read-only, which is the "could not
    /// handle it" case, so it is answered with
    /// [`CapabilityAccessError::OutOfRange`] rather than a success that did
    /// nothing. `append_line` takes the line to append and `set_text` / `set_cursor`
    /// assign state through the property route, so those are refused the same way
    /// — the names are right and the payload is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        let handled = match name {
            "undo" => self.undo(),
            "redo" => self.redo(),
            "copy" => self.copy(),
            "cut" => self.cut(),
            "paste" => self.paste(),
            "select_all" => {
                self.select_all();
                true
            }
            "find" => {
                self.open_find(false);
                true
            }
            "replace" => {
                self.open_find(true);
                true
            }
            "find_next" => self.find_next(false),
            "find_previous" => self.find_next(true),
            "toggle_comment" => {
                self.toggle_line_comment();
                true
            }
            "toggle_block_comment" => {
                self.toggle_block_comment();
                true
            }
            "duplicate_line" => {
                self.duplicate_line();
                true
            }
            "delete_line" => {
                self.delete_line();
                true
            }
            "indent" => {
                self.indent_selection();
                true
            }
            "outdent" => {
                self.outdent_selection();
                true
            }
            "sort_lines" => {
                self.sort_lines();
                true
            }
            "fold_all" => self.fold_all() > 0,
            "unfold_all" => {
                self.unfold_all();
                true
            }
            "complete" => self.trigger_completion() > 0,
            "close_find" => {
                self.close_find();
                true
            }
            _ => return self.command_with_payload(name),
        };
        if handled {
            Ok(())
        } else {
            Err(CapabilityAccessError::OutOfRange)
        }
    }
}

impl CodeEditor {
    /// Answers the commands that take their value through the property route.
    fn command_with_payload(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "append_line" | "set_text" | "set_markers" | "set_cursor" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
