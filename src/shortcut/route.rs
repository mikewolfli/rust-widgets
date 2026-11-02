// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Window-scoped key routing, ahead of the focused control.
//!
//! # Why this lives in the shortcut module
//!
//! The event loop already has a key hook ([`crate::event::r#loop::EventShortcutFn`]) but its
//! signature carries only the event, so it cannot tell *which window* a key belongs to. A
//! `WindowHandle::set_key_handler` registration needs exactly that: the key is offered to the
//! handler for the top-level window the event was routed through, and only if that handler
//! declines does the key continue to the focused control. Keeping the registry here means the
//! loop consults one place and the handle layer writes one place.
//!
//! # Text input and IME are not commands
//!
//! Three rules keep a bare character from becoming a command:
//!
//! 1. only [`Event::KeyPress`] is offered — `TextInput`, `ImePreedit` and `ImeCommit` never are;
//! 2. while an input method is composing ([`observe_input_event`] tracks the preedit stream),
//!    no key is offered at all;
//! 3. a text-producing key with no `CTRL`/`ALT`/`META`/`PRIMARY` modifier is left to the
//!    focused control, because `S` is text and `Shift+S` is still text.
//!
//! # Unhandled keys continue
//!
//! A handler returning `false` is not a veto of the key; the loop then runs its existing
//! accelerator hook and delivers the key to the focused control. That is the only ordering
//! that lets a window own `Ctrl+S` while every other key still reaches the editor.

use crate::compat::{Box, Cell, HashMap, RefCell};
use crate::core::ObjectId;
use crate::event::{Event, FocusInfo};

use super::{Key, Modifiers, PlatformShortcutStyle, Shortcut};

/// The callback a window registers for window-level key routing.
///
/// It is called on the UI thread and must not retain the borrowed values.
pub type WindowKeyHandler = Box<dyn Fn(&Shortcut, &FocusInfo) -> bool>;

thread_local! {
    /// Installed handlers, keyed by top-level window id.
    static KEY_HANDLERS: RefCell<HashMap<ObjectId, WindowKeyHandler>> =
        RefCell::new(HashMap::new());

    /// Whether an input method is currently composing.
    ///
    /// Driven by the preedit/commit stream rather than by a platform query, so the answer is
    /// the same on every backend and needs no IME bridge to exist.
    static IME_COMPOSING: Cell<bool> = const { Cell::new(false) };
}

/// Installs `handler` as the window-level key handler for `window_id`, replacing any previous
/// one for that window.
pub(crate) fn set_key_handler(window_id: ObjectId, handler: WindowKeyHandler) {
    KEY_HANDLERS.with(|handlers| {
        handlers.borrow_mut().insert(window_id, handler);
    });
}

/// Removes the window-level key handler for `window_id`, returning whether one was installed.
pub(crate) fn clear_key_handler(window_id: ObjectId) -> bool {
    KEY_HANDLERS.with(|handlers| handlers.borrow_mut().remove(&window_id).is_some())
}

/// Updates the composition flag from the input stream.
///
/// A non-empty preedit opens a composition; an empty preedit or a commit closes it. Every
/// other event is ignored, so a stream that simply stops sending preedits does not leave the
/// flag stuck forever once a commit (the normal end of a composition) arrives.
pub(crate) fn observe_input_event(event: &Event) {
    match event {
        Event::ImePreedit { text, .. } => {
            IME_COMPOSING.with(|composing| composing.set(!text.is_empty()));
        }
        Event::ImeCommit { .. } => {
            IME_COMPOSING.with(|composing| composing.set(false));
        }
        _ => {}
    }
}

/// Offers `event` to the top-level window's key handler.
///
/// Returns `true` when the handler claimed the key; the caller must then skip the accelerator
/// hook and control delivery. Every other case — a non-key event, a bare character, an active
/// composition, no handler, or a handler that declined — returns `false` and the key continues.
pub(crate) fn dispatch_window_key(target: ObjectId, event: &Event) -> bool {
    let Event::KeyPress { key, modifiers } = event else {
        return false;
    };
    let Some(key) = Key::from_key_code(*key) else {
        return false;
    };
    let focused = crate::widget::runtime::focused_widget();
    let Some(window) = crate::widget::runtime::top_level_window_of(target)
        .or_else(|| focused.and_then(crate::widget::runtime::top_level_window_of))
    else {
        return false;
    };
    let composing = IME_COMPOSING.with(|composing| composing.get());
    if composing {
        return false;
    }
    let shortcut = canonical_shortcut(key, *modifiers);
    if !has_command_modifier(shortcut.modifiers) && key.is_text_producing() {
        return false;
    }
    let text_input = focused
        .and_then(|id| {
            crate::widget::runtime::read_widget(id, |widget| widget.input_method_query().is_some())
        })
        .unwrap_or(false);
    let info = FocusInfo::new(focused, Some(window))
        .with_text_input(text_input)
        .with_ime_composition(composing);
    KEY_HANDLERS.with(|handlers| {
        handlers.borrow().get(&window).map(|handler| handler(&shortcut, &info)).unwrap_or(false)
    })
}

/// Whether a modifier set makes the chord a command rather than text.
fn has_command_modifier(modifiers: Modifiers) -> bool {
    modifiers.contains(Modifiers::CTRL)
        || modifiers.contains(Modifiers::ALT)
        || modifiers.contains(Modifiers::META)
        || modifiers.contains(Modifiers::PRIMARY)
}

/// Normalises the event's modifier bits to the platform's shortcut vocabulary.
///
/// The event's Meta bit already becomes `PRIMARY`, so the remaining work is the platform
/// convention around Control: on macOS `Command` also reports the physical Control bit, which
/// must not split `⌘S` from its binding; on Windows/Linux Control *is* the platform
/// accelerator, so a Control press is lifted to `PRIMARY` for the same reason. A physical
/// Control press on macOS stays `CTRL`, which is the one platform where the two keys differ.
fn canonical_shortcut(key: Key, bits: u32) -> Shortcut {
    canonical_shortcut_for_style(key, bits, crate::platform::platform_facts().shortcut_style())
}

/// The style-parameterised body of [`canonical_shortcut`], so the unit tests can exercise both
/// platform conventions on any host.
fn canonical_shortcut_for_style(key: Key, bits: u32, style: PlatformShortcutStyle) -> Shortcut {
    let mut modifiers = Modifiers::from_event_bits(bits);
    if modifiers.contains(Modifiers::PRIMARY) {
        // macOS reports Control alongside Command on some events; the binding identity is the
        // primary accelerator, not the incidental physical key.
        modifiers = modifiers.without(Modifiers::CTRL);
    } else if style == PlatformShortcutStyle::Desktop {
        // On Windows/Linux Control is the accelerator, so `Shortcut::primary` must answer to it.
        modifiers = modifiers.without(Modifiers::CTRL) | Modifiers::PRIMARY;
    }
    Shortcut::new(key, modifiers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{String, ToString};
    use crate::event::Event;
    use crate::shortcut::Modifiers;

    #[test]
    fn a_bare_letter_and_shift_letter_are_text_not_commands() {
        assert!(!has_command_modifier(Modifiers::NONE));
        assert!(!has_command_modifier(Modifiers::SHIFT));
        assert!(Key::S.is_text_producing());
        assert!(Key::Num7.is_text_producing());
        assert!(Key::Space.is_text_producing());
        assert!(!Key::Tab.is_text_producing());
        assert!(!Key::Enter.is_text_producing());
        assert!(!Key::Escape.is_text_producing());
        assert!(!Key::F5.is_text_producing());
    }

    #[test]
    fn command_modifiers_make_a_chord_routable() {
        assert!(has_command_modifier(Modifiers::PRIMARY));
        assert!(has_command_modifier(Modifiers::CTRL));
        assert!(has_command_modifier(Modifiers::ALT));
        assert!(has_command_modifier(Modifiers::META));
        assert!(has_command_modifier(Modifiers::PRIMARY | Modifiers::SHIFT));
    }

    #[test]
    fn the_desktop_style_lifts_control_to_primary() {
        // Event bit 1 is Control.
        let shortcut = canonical_shortcut_for_style(Key::S, 0b0010, PlatformShortcutStyle::Desktop);
        assert_eq!(shortcut, Shortcut::primary(Key::S));
    }

    #[test]
    fn the_mac_style_drops_the_incidental_control_bit() {
        // Event bit 3 is Command; `from_event_bits` also sets Control for it.
        let shortcut = canonical_shortcut_for_style(Key::S, 0b1000, PlatformShortcutStyle::Mac);
        assert_eq!(shortcut, Shortcut::primary(Key::S));
        assert!(!shortcut.modifiers.contains(Modifiers::CTRL));
    }

    #[test]
    fn a_physical_control_press_stays_control_on_mac() {
        let shortcut = canonical_shortcut_for_style(Key::C, 0b0010, PlatformShortcutStyle::Mac);
        assert_eq!(shortcut, Shortcut::ctrl(Key::C));
        assert!(!shortcut.modifiers.contains(Modifiers::PRIMARY));
    }

    #[test]
    fn shift_is_preserved_alongside_the_primary_configured_by_style() {
        // Shift + Control on the desktop convention.
        let shortcut = canonical_shortcut_for_style(Key::Z, 0b0011, PlatformShortcutStyle::Desktop);
        assert_eq!(shortcut, Shortcut::primary_shift(Key::Z));
    }

    #[test]
    fn preedit_opens_a_composition_and_commit_closes_it() {
        observe_input_event(&Event::ImePreedit { text: "に".to_string(), cursor: 1 });
        assert!(IME_COMPOSING.with(|composing| composing.get()));
        observe_input_event(&Event::ImePreedit { text: String::new(), cursor: 0 });
        assert!(!IME_COMPOSING.with(|composing| composing.get()));

        observe_input_event(&Event::ImePreedit { text: "に".to_string(), cursor: 1 });
        observe_input_event(&Event::ImeCommit { text: "に".to_string() });
        assert!(!IME_COMPOSING.with(|composing| composing.get()));
    }
}
