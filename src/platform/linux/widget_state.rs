// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::types::{LinuxHandleKind, LinuxPlatform};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::core::MutexExt;
use crate::platform::{DropEvent, WidgetTriggerEvent, WidgetTriggerKind};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use gtk::prelude::*;

impl LinuxPlatform {
    pub(crate) fn show_widget_impl(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(window) = native.windows.get(&widget_id) {
                window.show_all();
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.show();
            }
        }
    }
    pub(crate) fn hide_widget_impl(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(window) = native.windows.get(&widget_id) {
                window.hide();
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.hide();
            }
        }
    }
    pub(crate) fn set_widget_geometry_impl(
        &self,
        widget_id: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) {
        self.state.set_geometry(widget_id, x, y, width, height);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let parent_id = {
                let menus = self.menus.lock().expect("linux menu lock poisoned");
                menus.widget_parent.get(&widget_id).copied()
            };
            let native = self.native.lock_guard();
            if let Some(window) = native.windows.get(&widget_id) {
                window.move_(x, y);
                window.resize(width as i32, height as i32);
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.set_size_request(width as i32, height as i32);
                if let Some(parent_id) = parent_id {
                    if let Some(container) = native.content_fixed.get(&parent_id) {
                        container.move_(widget, x, y);
                    }
                }
            }
        }
    }
    pub(crate) fn set_widget_text_impl(&self, widget_id: u64, text: &str) {
        if !self.state.set_text(widget_id, text) {
            return;
        }
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(window) = native.windows.get(&widget_id) {
                window.set_title(text);
            } else if let Some(widget) = native.widgets.get(&widget_id) {
                // Order matters: GTK subclasses must be checked before their base
                // classes, otherwise the more generic branch wins. `SpinButton`
                // extends `Entry`, and `CheckButton`/`RadioButton` extend
                // `Button`, so those are matched first.
                if let Ok(menu_item) = widget.clone().downcast::<gtk::MenuItem>() {
                    menu_item.set_label(text);
                } else if let Ok(spin) = widget.clone().downcast::<gtk::SpinButton>() {
                    // A numeric entry only accepts parseable input; ignore the
                    // update when the text is not a number rather than clearing
                    // the current value.
                    if let Ok(value) = text.trim().parse::<f64>() {
                        spin.set_value(value);
                    }
                } else if let Ok(check) = widget.clone().downcast::<gtk::CheckButton>() {
                    check.set_label(text);
                } else if let Ok(radio) = widget.clone().downcast::<gtk::RadioButton>() {
                    radio.set_label(text);
                } else if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
                    button.set_label(text);
                } else if let Ok(entry) = widget.clone().downcast::<gtk::Entry>() {
                    entry.set_text(text);
                } else if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
                    label.set_text(text);
                } else if let Ok(frame) = widget.clone().downcast::<gtk::Frame>() {
                    // Panels are rendered as frames; surface the text as a label
                    // so `set_widget_text` is not a silent no-op for them.
                    frame.set_label(Some(text));
                } else if let Ok(progress) = widget.clone().downcast::<gtk::ProgressBar>() {
                    if let Ok(value) = text.trim().parse::<f64>() {
                        progress.set_fraction(value.clamp(0.0, 1.0));
                    } else {
                        progress.set_show_text(true);
                        progress.set_text(Some(text));
                    }
                }
            }
        }
        if matches!(self.kind_of(widget_id), Some(LinuxHandleKind::LineEdit)) {
            self.menus
                .lock()
                .expect("linux menu lock poisoned")
                .pending_widget_events
                .push_back(WidgetTriggerEvent { widget_id, kind: WidgetTriggerKind::ValueChanged });
        }
    }
    pub(crate) fn get_widget_text_impl(&self, widget_id: u64) -> String {
        self.state.text(widget_id)
    }
    pub(crate) fn set_widget_enabled_impl(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.set_sensitive(enabled);
            }
        }
    }
    pub(crate) fn is_widget_enabled_impl(&self, widget_id: u64) -> bool {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                return widget.is_sensitive();
            }
        }
        self.state.enabled(widget_id)
    }
    pub(crate) fn set_widget_visible_impl(&self, widget_id: u64, visible: bool) {
        self.state.set_visible(widget_id, visible);
        if visible {
            self.show_widget_impl(widget_id);
        } else {
            self.hide_widget_impl(widget_id);
        }
    }
    pub(crate) fn is_widget_visible_impl(&self, widget_id: u64) -> bool {
        // The logical state is authoritative: `show_widget_impl`/`hide_widget_impl`
        // always update it, then mirror the change onto the GTK widget. GTK's
        // `is_visible()` only becomes true once the widget is partially realized
        // (its top-level mapped), so querying it here would report a freshly
        // `show()`-ed control as hidden before the first display round-trip and
        // disagree with the value just written by `set_widget_visible`.
        self.state.visible(widget_id)
    }

    /// Whether this Linux handle kind carries a primary numeric value.
    ///
    /// The backend's own answer about the GTK widgets it creates: GTK stores them
    /// type-erased in `native.widgets`, so the recorded kind is what selects the
    /// correct downcast.
    pub(crate) fn kind_accepts_numeric_value(kind: LinuxHandleKind) -> bool {
        matches!(
            kind,
            LinuxHandleKind::Slider
                | LinuxHandleKind::ProgressBar
                | LinuxHandleKind::SpinBox
                | LinuxHandleKind::DoubleSpinBox
                | LinuxHandleKind::ScrollBar
                | LinuxHandleKind::ActivityIndicator
                | LinuxHandleKind::ProgressDialog
        )
    }

    pub(crate) fn set_widget_value_impl(&self, widget_id: u64, value: f64) -> bool {
        let Some(kind) = self.kind_of(widget_id) else {
            return false;
        };
        if !Self::kind_accepts_numeric_value(kind) {
            return false;
        }
        self.state.set_value(widget_id, value);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(scale) = widget.clone().downcast::<gtk::Scale>() {
                    scale.set_value(value);
                } else if let Ok(spin) = widget.clone().downcast::<gtk::SpinButton>() {
                    spin.set_value(value);
                } else if let Ok(progress) = widget.clone().downcast::<gtk::ProgressBar>() {
                    // GtkProgressBar's natural model is a 0..=1 fraction, while the
                    // rest of the crate uses an arbitrary numeric value; convert so
                    // the two agree with the value stored in `state`.
                    let (min, max) = self.state.range(widget_id).unwrap_or((0.0, 100.0));
                    let span = (max - min).abs().max(f64::EPSILON);
                    progress.set_fraction(((value - min) / span).clamp(0.0, 1.0));
                } else if let Ok(adjustment) = widget.clone().downcast::<gtk::Scrollbar>() {
                    adjustment.adjustment().set_value(value);
                }
            }
        }
        true
    }

    pub(crate) fn widget_value_impl(&self, widget_id: u64) -> Option<f64> {
        let kind = self.kind_of(widget_id)?;
        if !Self::kind_accepts_numeric_value(kind) {
            return None;
        }
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(scale) = widget.clone().downcast::<gtk::Scale>() {
                    return Some(scale.value());
                }
                if let Ok(spin) = widget.clone().downcast::<gtk::SpinButton>() {
                    return Some(spin.value());
                }
            }
        }
        self.state.value(widget_id)
    }

    pub(crate) fn set_widget_range_impl(&self, widget_id: u64, min: f64, max: f64) -> bool {
        let Some(kind) = self.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, LinuxHandleKind::Slider | LinuxHandleKind::SpinBox) {
            return false;
        }
        self.state.set_range(widget_id, min, max);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(scale) = widget.clone().downcast::<gtk::Scale>() {
                    let adjustment = scale.adjustment();
                    adjustment.set_lower(min);
                    adjustment.set_upper(max);
                } else if let Ok(spin) = widget.clone().downcast::<gtk::SpinButton>() {
                    let adjustment = spin.adjustment();
                    adjustment.set_lower(min);
                    adjustment.set_upper(max);
                }
            }
        }
        true
    }

    pub(crate) fn widget_range_impl(&self, widget_id: u64) -> Option<(f64, f64)> {
        let kind = self.kind_of(widget_id)?;
        if !matches!(kind, LinuxHandleKind::Slider | LinuxHandleKind::SpinBox) {
            return None;
        }
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(scale) = widget.clone().downcast::<gtk::Scale>() {
                    let adjustment = scale.adjustment();
                    return Some((adjustment.lower(), adjustment.upper()));
                }
                if let Ok(spin) = widget.clone().downcast::<gtk::SpinButton>() {
                    let adjustment = spin.adjustment();
                    return Some((adjustment.lower(), adjustment.upper()));
                }
            }
        }
        self.state.range(widget_id)
    }

    pub(crate) fn set_widget_selected_index_impl(
        &self,
        widget_id: u64,
        index: Option<usize>,
    ) -> bool {
        let Some(kind) = self.kind_of(widget_id) else {
            return false;
        };
        match (kind, index) {
            (LinuxHandleKind::ComboBox, Some(selected)) => {
                // Reuse the specialised path so the item table and bounds check stay
                // authoritative.
                crate::platform::Platform::combo_box_set_current_index(self, widget_id, selected)
            }
            (LinuxHandleKind::ListBox, Some(selected)) => {
                crate::platform::Platform::list_box_set_current_index(self, widget_id, selected)
            }
            (LinuxHandleKind::ComboBox | LinuxHandleKind::ListBox, None) => {
                // GTK selection models do not represent "cleared"; report the
                // limitation rather than faking a selection change.
                false
            }
            _ => false,
        }
    }

    pub(crate) fn widget_selected_index_impl(&self, widget_id: u64) -> Option<usize> {
        match self.kind_of(widget_id)? {
            LinuxHandleKind::ComboBox => {
                crate::platform::Platform::combo_box_current_index(self, widget_id)
            }
            LinuxHandleKind::ListBox => {
                crate::platform::Platform::list_box_current_index(self, widget_id)
            }
            _ => None,
        }
    }

    pub(crate) fn set_widget_checked_impl(&self, widget_id: u64, checked: bool) -> bool {
        let Some(kind) = self.kind_of(widget_id) else {
            return false;
        };
        if !matches!(
            kind,
            LinuxHandleKind::CheckBox
                | LinuxHandleKind::RadioButton
                | LinuxHandleKind::ToggleButton
        ) {
            return false;
        }
        self.state.set_checked(widget_id, checked);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(check) = widget.clone().downcast::<gtk::ToggleButton>() {
                    check.set_active(checked);
                }
            }
        }
        true
    }

    pub(crate) fn is_widget_checked_impl(&self, widget_id: u64) -> Option<bool> {
        let kind = self.kind_of(widget_id)?;
        if !matches!(
            kind,
            LinuxHandleKind::CheckBox
                | LinuxHandleKind::RadioButton
                | LinuxHandleKind::ToggleButton
        ) {
            return None;
        }
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(check) = widget.clone().downcast::<gtk::ToggleButton>() {
                    return Some(check.is_active());
                }
            }
        }
        self.state.checked(widget_id)
    }

    pub(crate) fn set_widget_step_impl(&self, widget_id: u64, step: f64) -> bool {
        let Some(kind) = self.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, LinuxHandleKind::Slider | LinuxHandleKind::SpinBox) {
            return false;
        }
        self.state.set_step(widget_id, step);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(scale) = widget.clone().downcast::<gtk::Scale>() {
                    let adjustment = scale.adjustment();
                    adjustment.set_step_increment(step);
                } else if let Ok(spin) = widget.clone().downcast::<gtk::SpinButton>() {
                    let adjustment = spin.adjustment();
                    adjustment.set_step_increment(step);
                }
            }
        }
        true
    }

    pub(crate) fn widget_step_impl(&self, widget_id: u64) -> Option<f64> {
        let kind = self.kind_of(widget_id)?;
        if !matches!(kind, LinuxHandleKind::Slider | LinuxHandleKind::SpinBox) {
            return None;
        }
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(scale) = widget.clone().downcast::<gtk::Scale>() {
                    return Some(scale.adjustment().step_increment());
                }
                if let Ok(spin) = widget.clone().downcast::<gtk::SpinButton>() {
                    return Some(spin.adjustment().step_increment());
                }
            }
        }
        self.state.step(widget_id)
    }

    pub(crate) fn set_widget_indeterminate_impl(
        &self,
        widget_id: u64,
        indeterminate: bool,
    ) -> bool {
        let Some(kind) = self.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, LinuxHandleKind::ProgressBar | LinuxHandleKind::ActivityIndicator) {
            return false;
        }
        self.state.set_indeterminate(widget_id, indeterminate);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(progress) = widget.clone().downcast::<gtk::ProgressBar>() {
                    // GTK has no persistent indeterminate flag: `pulse()` advances
                    // the bar a step, and a timer pulsing it is what produces the
                    // animation. Setting pulse_step lets the application's own
                    // pulse cadence look like a marquee rather than a jump.
                    if indeterminate {
                        progress.pulse();
                    } else {
                        progress.set_fraction(0.0);
                    }
                }
            }
        }
        true
    }

    pub(crate) fn is_widget_indeterminate_impl(&self, widget_id: u64) -> Option<bool> {
        let kind = self.kind_of(widget_id)?;
        if !matches!(kind, LinuxHandleKind::ProgressBar | LinuxHandleKind::ActivityIndicator) {
            return None;
        }
        self.state.indeterminate(widget_id)
    }

    pub(crate) fn set_widget_read_only_impl(&self, widget_id: u64, read_only: bool) -> bool {
        let Some(kind) = self.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, LinuxHandleKind::LineEdit) {
            return false;
        }
        self.state.set_read_only(widget_id, read_only);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(entry) = widget.clone().downcast::<gtk::Entry>() {
                    entry.set_editable(!read_only);
                }
            }
        }
        true
    }

    pub(crate) fn is_widget_read_only_impl(&self, widget_id: u64) -> Option<bool> {
        let kind = self.kind_of(widget_id)?;
        if !matches!(kind, LinuxHandleKind::LineEdit) {
            return None;
        }
        self.state.read_only(widget_id)
    }

    pub(crate) fn set_widget_max_length_impl(&self, widget_id: u64, max_length: u32) -> bool {
        let Some(kind) = self.kind_of(widget_id) else {
            return false;
        };
        if !matches!(kind, LinuxHandleKind::LineEdit) {
            return false;
        }
        self.state.set_max_length(widget_id, max_length);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(entry) = widget.clone().downcast::<gtk::Entry>() {
                    // GTK models "unlimited" as -1; a u32 sentinel of 0 is not a
                    // real limit, so only apply a plausible positive bound.
                    let limit = if max_length >= i32::MAX as u32 { -1 } else { max_length as i32 };
                    entry.set_max_length(limit);
                }
            }
        }
        true
    }

    pub(crate) fn widget_max_length_impl(&self, widget_id: u64) -> Option<u32> {
        let kind = self.kind_of(widget_id)?;
        if !matches!(kind, LinuxHandleKind::LineEdit) {
            return None;
        }
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock_guard();
            if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(entry) = widget.clone().downcast::<gtk::Entry>() {
                    let limit = entry.max_length();
                    return Some(if limit < 0 { u32::MAX } else { limit as u32 });
                }
            }
        }
        self.state.max_length(widget_id)
    }

    pub(crate) fn set_widget_ime_enabled_impl(&self, widget_id: u64, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }
    pub(crate) fn is_widget_ime_enabled_impl(&self, widget_id: u64) -> bool {
        self.state.ime_enabled(widget_id)
    }
    pub(crate) fn set_widget_accessibility_name_impl(&self, widget_id: u64, name: &str) -> bool {
        if !self.state.set_accessibility_name(widget_id, name) {
            return false;
        }
        // Mirror the name onto the AT-SPI bridge and notify screen readers so
        // the change is observable, not just recorded in logical state.
        #[cfg(target_os = "linux")]
        if let Some(bridge) = crate::platform::Platform::accessibility_bridge(self) {
            bridge.set_accessibility_name(widget_id, name);
            bridge.notify_name_changed(widget_id);
        }
        true
    }
    pub(crate) fn get_widget_accessibility_name_impl(&self, widget_id: u64) -> String {
        // The AT-SPI bridge is the authoritative store when it knows the widget;
        // fall back to logical state otherwise.
        #[cfg(target_os = "linux")]
        if let Some(bridge) = crate::platform::Platform::accessibility_bridge(self) {
            if let Some(name) = bridge.accessibility_name(widget_id) {
                return name;
            }
        }
        self.state.accessibility_name(widget_id)
    }
    pub(crate) fn set_clipboard_text_impl(&self, text: &str) -> bool {
        // Mirror into the logical state so reads work even when the display
        // clipboard is unavailable (headless / no GDK display).
        let stored = self.state.set_clipboard_text(text);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            if let Some(clipboard) = gtk_clipboard() {
                clipboard.set_text(text);
                // `store()` hands ownership of the selection to the clipboard
                // manager so the contents survive after this process exits.
                clipboard.store();
                return true;
            }
        }
        stored
    }
    pub(crate) fn get_clipboard_text_impl(&self) -> String {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // Prefer the real display clipboard when GTK is initialized and a
            // display is reachable; fall back to the logical mirror otherwise.
            if let Some(clipboard) = gtk_clipboard() {
                if let Some(text) = clipboard.wait_for_text() {
                    let text = text.to_string();
                    // Keep the logical mirror in sync with the native source.
                    self.state.set_clipboard_text(&text);
                    return text;
                }
            }
        }
        self.state.clipboard_text()
    }
    pub(crate) fn begin_drag_impl(
        &self,
        source_widget_id: u64,
        mime: &str,
        payload: &[u8],
    ) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }
    pub(crate) fn poll_drop_event_impl(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }
    pub(crate) fn inject_drop_event_impl(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }
    pub(crate) fn create_message_box_impl(
        &self,
        parent: u64,
        title: &str,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // Record the title alongside the body so the state backend retains the
        // full dialog identity even when the native GTK path is compiled out.
        let id = self.insert_widget(
            LinuxHandleKind::MessageBox,
            &format!("{}: {}", title, text),
            x,
            y,
            width,
            height,
        );
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // GTK message dialogs need a transient parent window; fall back to a
            // parentless dialog when the caller passed an invalid or non-window id
            // so the native object still exists instead of being silently skipped.
            let parent_window = self.native_parent_window(parent);
            let dialog = gtk::MessageDialog::new(
                parent_window.as_ref(),
                gtk::DialogFlags::MODAL,
                gtk::MessageType::Info,
                gtk::ButtonsType::Ok,
                text,
            );
            dialog.set_title(title);
            dialog.set_default_size(width as i32, height as i32);
            dialog.connect_response(|dialog, _| {
                // Closing on any response keeps the dialog from leaking an open
                // modal loop when no application handler is registered.
                dialog.close();
            });
            let mut native = self.native.lock_guard();
            native.widgets.insert(id, dialog.clone().upcast::<gtk::Widget>());
            native.dialogs.insert(id, dialog.upcast::<gtk::Dialog>());
        }
        id
    }
    pub(crate) fn create_file_dialog_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        let id = self.insert_widget(LinuxHandleKind::FileDialog, "FileDialog", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let parent_window = self.native_parent_window(parent);
            let dialog = gtk::FileChooserDialog::new(
                Some("File"),
                parent_window.as_ref(),
                gtk::FileChooserAction::Open,
            );
            dialog.add_buttons(&[
                ("Cancel", gtk::ResponseType::Cancel),
                ("Open", gtk::ResponseType::Accept),
            ]);
            dialog.set_default_size(width as i32, height as i32);
            let mut native = self.native.lock_guard();
            native.widgets.insert(id, dialog.clone().upcast::<gtk::Widget>());
            native.dialogs.insert(id, dialog.upcast::<gtk::Dialog>());
        }
        id
    }
    pub(crate) fn create_color_dialog_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        let id =
            self.insert_widget(LinuxHandleKind::ColorDialog, "ColorDialog", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let parent_window = self.native_parent_window(parent);
            let dialog = gtk::ColorChooserDialog::new(Some("Color"), parent_window.as_ref());
            dialog.set_default_size(width as i32, height as i32);
            let mut native = self.native.lock_guard();
            native.widgets.insert(id, dialog.clone().upcast::<gtk::Widget>());
            native.color_choosers.insert(id, dialog.clone().upcast::<gtk::ColorChooser>());
            native.dialogs.insert(id, dialog.upcast::<gtk::Dialog>());
        }
        id
    }
    pub(crate) fn create_font_dialog_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        let id = self.insert_widget(LinuxHandleKind::FontDialog, "FontDialog", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let parent_window = self.native_parent_window(parent);
            let dialog = gtk::FontChooserDialog::new(Some("Font"), parent_window.as_ref());
            dialog.set_default_size(width as i32, height as i32);
            let mut native = self.native.lock_guard();
            native.widgets.insert(id, dialog.clone().upcast::<gtk::Widget>());
            native.font_choosers.insert(id, dialog.clone().upcast::<gtk::FontChooser>());
            native.dialogs.insert(id, dialog.upcast::<gtk::Dialog>());
        }
        id
    }

    /// Resolve the native GTK window that should act as the transient parent for a
    /// dialog. Returns `None` when the id is unknown or not a window, which makes
    /// the dialog parentless rather than failing to construct it.
    #[cfg(all(target_os = "linux", feature = "gtk-native"))]
    pub(crate) fn native_parent_window(&self, parent: u64) -> Option<gtk::Window> {
        let native = self.native.lock_guard();
        native.windows.get(&parent).cloned()
    }
}

/// Obtain the default GDK display clipboard.
///
/// Returns `None` when GTK was not initialized on the calling thread, or when
/// no GDK display is reachable (headless CI). Callers fall back to the
/// in-process clipboard mirror instead of panicking.
///
/// The main-thread check is load-bearing: `gdk::Display::default()` **panics**
/// (rather than returning `None`) when touched from a thread other than the one
/// that ran `gtk::init()`. Without this guard, reading or writing the clipboard
/// from any non-GTK thread — for example a `cargo test` worker thread — would
/// abort the process with "GDK may only be used from the main thread".
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
fn gtk_clipboard() -> Option<gtk::Clipboard> {
    if !gtk::is_initialized_main_thread() {
        return None;
    }
    let display = gtk::gdk::Display::default()?;
    gtk::Clipboard::default(&display)
}
