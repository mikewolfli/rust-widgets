use super::types::{LinuxHandleKind, LinuxPlatform};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::core::MutexExt;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use gtk::prelude::*;
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use std::sync::Arc;

/// A parsed menu accelerator: the GTK key binding plus the text to display.
struct ParsedAccelerator {
    /// GTK key value and modifier mask, present when the chord can be bound.
    ///
    /// The key value is the raw `u32` GDK keyval, which is what
    /// `gtk_widget_add_accelerator` expects.
    binding: Option<(u32, gdk::ModifierType)>,
    /// Text to show next to the label; `None` when nothing was supplied.
    display: Option<String>,
}

/// Resolves a key token to a GDK keyval.
///
/// Handles the named keys whose spelling differs between the framework and GDK,
/// then falls back to GDK's own name lookup (`"F1"`, `"Home"`, single letters),
/// so a token does not need a hand-written arm just to become a keyval.
fn keyval_for_token(token: &str) -> Option<u32> {
    use gdk::keys::constants as gdk_key;
    let named = match token {
        "backspace" | "del" | "delete" => Some(gdk_key::BackSpace),
        "esc" | "escape" => Some(gdk_key::Escape),
        "enter" | "return" => Some(gdk_key::Return),
        "space" => Some(gdk_key::space),
        "tab" => Some(gdk_key::Tab),
        "insert" | "ins" => Some(gdk_key::Insert),
        "home" => Some(gdk_key::Home),
        "end" => Some(gdk_key::End),
        "pageup" | "pgup" => Some(gdk_key::Page_Up),
        "pagedown" | "pgdn" => Some(gdk_key::Page_Down),
        "left" => Some(gdk_key::Left),
        "right" => Some(gdk_key::Right),
        "up" => Some(gdk_key::Up),
        "down" => Some(gdk_key::Down),
        _ => None,
    };
    if let Some(key) = named {
        return Some(*key);
    }
    if token.is_empty() {
        return None;
    }
    // GDK understands "F1", "Home", and single characters by name. `from_name`
    // returns keyval 0 for anything it does not know, which is treated as a miss
    // so an unrecognised token does not silently bind the "no key" value.
    let keyval = *gdk::keys::Key::from_name(token);
    if keyval == 0 {
        None
    } else {
        Some(keyval)
    }
}

/// Parses displayed accelerator text (e.g. `"Ctrl+Shift+Z"`) for GTK.
///
/// Accepts the spelled-out desktop notation as well as the macOS glyph form, so
/// text produced by `Platform::format_shortcut` on any host can be passed
/// straight through. Modifier name mismatches ("Cmd" on Linux) resolve to
/// Control, matching what the desktop style displays.
fn parse_accelerator(shortcut: Option<&str>) -> ParsedAccelerator {
    let Some(raw) = shortcut.map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return ParsedAccelerator { binding: None, display: None };
    };
    let mut modifiers = gdk::ModifierType::empty();
    let mut keyval = None;
    // Glyph forms are single characters rather than `+`-separated tokens, so they
    // are folded in before the token loop instead of being matched as tokens.
    for ch in raw.chars() {
        match ch {
            '⌘' => modifiers |= gdk::ModifierType::CONTROL_MASK,
            '⌃' | '^' => modifiers |= gdk::ModifierType::CONTROL_MASK,
            '⌥' => modifiers |= gdk::ModifierType::MOD1_MASK,
            '⇧' => modifiers |= gdk::ModifierType::SHIFT_MASK,
            '↩' => keyval = Some(*gdk::keys::constants::Return),
            '⎋' => keyval = Some(*gdk::keys::constants::Escape),
            '⌫' => keyval = Some(*gdk::keys::constants::BackSpace),
            _ => {}
        }
    }
    for part in raw.split('+') {
        let token = part.trim().to_lowercase();
        // A token may still carry a leading glyph; strip it before matching.
        let token = token.trim_start_matches(['⌘', '⌃', '^', '⌥', '⇧']).to_string();
        match token.as_str() {
            "primary" | "cmdorctrl" | "cmd" | "command" | "ctrl" | "control" => {
                modifiers |= gdk::ModifierType::CONTROL_MASK
            }
            "alt" | "option" => modifiers |= gdk::ModifierType::MOD1_MASK,
            "shift" => modifiers |= gdk::ModifierType::SHIFT_MASK,
            "super" | "meta" | "win" => modifiers |= gdk::ModifierType::SUPER_MASK,
            // Lone glyph tokens carry no key of their own.
            "⌘" | "⌃" | "⌥" | "⇧" | "↩" | "⎋" | "⌫" => {}
            other => {
                if let Some(resolved) = keyval_for_token(other) {
                    keyval = Some(resolved);
                }
            }
        }
    }
    ParsedAccelerator {
        binding: keyval.map(|key| (key, modifiers)),
        display: Some(raw.to_string()),
    }
}

/// Returns the accelerator display text, or an empty string when absent.
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
fn display_or_empty(parsed: &ParsedAccelerator) -> String {
    parsed.display.clone().unwrap_or_default()
}

impl LinuxPlatform {
    pub(crate) fn create_menu_bar_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if !matches!(self.kind_of(parent), Some(LinuxHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::MenuBar, "MenuBar", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu_bar = gtk::MenuBar::new();
            menu_bar.set_size_request(width as i32, height as i32);
            let widget = menu_bar.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            native.menu_bars.insert(id, menu_bar);
            native.widgets.insert(id, widget);
            let _ = (x, y);
        }
        id
    }
    pub(crate) fn create_menu_impl(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if !matches!(self.kind_of(parent), Some(LinuxHandleKind::MenuBar | LinuxHandleKind::Menu)) {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::Menu, text, x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .menu_children
            .entry(parent)
            .or_default()
            .push(id);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu = gtk::Menu::new();
            let menu_item = gtk::MenuItem::with_label(text);
            menu_item.set_submenu(Some(&menu));
            let mut native = self.native.lock_guard();
            if let Some(menu_bar) = native.menu_bars.get(&parent) {
                menu_bar.append(&menu_item);
            } else if let Some(parent_menu) = native.menus.get(&parent) {
                parent_menu.append(&menu_item);
            }
            native.widgets.insert(id, menu_item.clone().upcast::<gtk::Widget>());
            native.menus.insert(id, menu);
            let _ = (x, y, width, height);
        }
        id
    }
    pub(crate) fn create_tool_bar_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if !matches!(self.kind_of(parent), Some(LinuxHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::ToolBar, "ToolBar", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            toolbar.set_size_request(width as i32, height as i32);
            let widget = toolbar.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&toolbar, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_status_bar_impl(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if !matches!(self.kind_of(parent), Some(LinuxHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::StatusBar, text, x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let label = gtk::Label::new(Some(text));
            label.set_size_request(width as i32, height as i32);
            let widget = label.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&label, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn attach_menu_bar_to_window_impl(&self, window: u64, menu_bar: u64) -> bool {
        if matches!(self.kind_of(window), Some(LinuxHandleKind::Window))
            && matches!(self.kind_of(menu_bar), Some(LinuxHandleKind::MenuBar))
        {
            self.menus
                .lock()
                .expect("linux menu lock poisoned")
                .attached_menu_bar
                .insert(window, menu_bar);
            #[cfg(all(target_os = "linux", feature = "gtk-native"))]
            {
                let native = self.native.lock_guard();
                if let (Some(root), Some(bar)) =
                    (native.root_boxes.get(&window), native.menu_bars.get(&menu_bar))
                {
                    root.pack_start(bar, false, false, 0);
                    root.reorder_child(bar, 0);
                    bar.show_all();
                }
            }
            return true;
        }
        false
    }
    pub(crate) fn menu_add_item_impl(
        &self,
        parent_menu: u64,
        text: &str,
        shortcut: Option<&str>,
    ) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(LinuxHandleKind::Menu)) {
            return 0;
        }
        let item_id = self.insert_widget(LinuxHandleKind::MenuItem, text, 0, 0, 0, 0);
        let parsed = parse_accelerator(shortcut);
        {
            let mut menus = self.menus.lock().expect("linux menu lock poisoned");
            menus.menu_children.entry(parent_menu).or_default().push(item_id);
            // Remember the accelerator so it can be reported back through
            // `menu_item_shortcut`, even on hosts without a usable GTK runtime.
            menus.menu_item_shortcuts.insert(item_id, parsed.display.clone().unwrap_or_default());
        }
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu_item = gtk::MenuItem::with_label(text);
            let menus_arc = Arc::clone(&self.menus);
            menu_item.connect_activate(move |_| {
                menus_arc
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_menu_events
                    .push_back(item_id);
            });
            let mut native = self.native.lock_guard();
            if let Some(parent) = native.menus.get(&parent_menu) {
                parent.append(&menu_item);
            }
            // Bind a real accelerator so the chord fires the item. Accelerator
            // groups are owned by a GtkWindow, so it is resolved through the
            // menu's owner; a menu that is not yet attached to a window is left
            // unbound rather than bound to the wrong window.
            if let Some((keyval, modifiers)) = parsed.binding {
                if let Some(window_id) = self.owner_window_of_menu(parent_menu) {
                    let group = native
                        .accel_groups
                        .entry(window_id)
                        .or_insert_with(gtk::AccelGroup::new)
                        .clone();
                    if let Some(window) = native.windows.get(&window_id) {
                        window.add_accel_group(&group);
                    }
                    menu_item.add_accelerator(
                        "activate",
                        &group,
                        keyval,
                        modifiers,
                        gtk::AccelFlags::VISIBLE,
                    );
                    // `add_accelerator` renders the chord itself, but the label is
                    // set explicitly so the notation matches what the host wrote
                    // (e.g. "Ctrl+Shift+Z" rather than GTK's own abbreviation).
                    if let Some(display) = parsed.display.as_deref() {
                        menu_item.set_label(&format!("{text}\t{display}"));
                    }
                    native.accel_paths.insert(item_id, display_or_empty(&parsed));
                }
            }
            native.widgets.insert(item_id, menu_item.clone().upcast::<gtk::Widget>());
        }
        item_id
    }

    /// Returns the window that owns `menu`, walking up through nested submenus.
    ///
    /// Accelerator groups belong to a `GtkWindow`, but a menu item is added to a
    /// `GtkMenu` that may sit several levels below the bar, so the ownership has
    /// to be resolved by walking the recorded parent links.
    #[cfg(all(target_os = "linux", feature = "gtk-native"))]
    fn owner_window_of_menu(&self, menu: u64) -> Option<u64> {
        let menus = self.menus.lock().expect("linux menu lock poisoned");
        let mut current = menu;
        // Bounded walk: a cycle in the recorded parents must not hang the UI.
        for _ in 0..64 {
            if matches!(self.kind_of(current), Some(LinuxHandleKind::Window)) {
                return Some(current);
            }
            match menus.widget_parent.get(&current) {
                Some(parent) => current = *parent,
                None => return None,
            }
        }
        None
    }
    pub(crate) fn poll_menu_triggered_impl(&self) -> Option<u64> {
        self.menus.lock().expect("linux menu lock poisoned").pending_menu_events.pop_front()
    }
    /// Returns the accelerator text bound to a menu item, or `None` when the item
    /// has no shortcut or is not a menu item.
    pub(crate) fn menu_item_shortcut_impl(&self, item_id: u64) -> Option<String> {
        if !matches!(self.kind_of(item_id), Some(LinuxHandleKind::MenuItem)) {
            return None;
        }
        let menus = self.menus.lock().expect("linux menu lock poisoned");
        menus.menu_item_shortcuts.get(&item_id).filter(|text| !text.is_empty()).cloned()
    }
    pub(crate) fn inject_menu_trigger_impl(&self, menu_item_id: u64) -> bool {
        if !matches!(self.kind_of(menu_item_id), Some(LinuxHandleKind::MenuItem)) {
            return false;
        }
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .pending_menu_events
            .push_back(menu_item_id);
        true
    }
    pub(crate) fn poll_widget_triggered_impl(&self) -> Option<u64> {
        self.poll_widget_trigger_event_impl().map(|event| event.widget_id)
    }
    pub(crate) fn poll_widget_trigger_event_impl(&self) -> Option<WidgetTriggerEvent> {
        self.menus.lock().expect("linux menu lock poisoned").pending_widget_events.pop_front()
    }
    pub(crate) fn inject_widget_trigger_event_impl(
        &self,
        widget_id: u64,
        kind: WidgetTriggerKind,
    ) -> bool {
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .pending_widget_events
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }
}
