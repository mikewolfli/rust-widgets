// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Android IME bridge — soft-keyboard control plus in-process composition state.
//!
//! # What winit does, and what is copied here
//!
//! winit's Android backend implements `request_ime_update(Enable/Disable)` by calling
//! `AndroidApp::show_soft_input` / `hide_soft_input`; it does **not** speak the Android
//! `GameTextInput` composition protocol, so winit makes no claim about preedit text. This bridge
//! follows the same split:
//!
//! * focus entering a widget asks the `InputMethodManager` to show the soft keyboard, and focus
//!   leaving asks it to hide the keyboard (`android_jni::show_soft_input` /
//!   `hide_soft_input`);
//! * the composition/marked-text state machine is kept in process (the same shape the Linux and
//!   Windows bridges use) so a caller that drives [`set_composition`](ImeBridge::set_composition)
//!   has somewhere to observe it;
//! * a commit is delivered to the focused widget as [`Event::ImeCommit`](crate::event::Event::ImeCommit)
//!   through [`deliver_commit`](crate::platform::ime::deliver_commit), so the widget layer sees
//!   the same event it would on every other backend.
//!
//! The soft-keyboard calls are real JNI calls against the host `Activity`'s `InputMethodManager`.
//! On a host build (or before `nativeAttachContext`) they return `false`, and
//! `AndroidPlatform::ime_bridge()` answers `None`, so the capability is not claimed before there
//! is anything to control.

use crate::compat::{lock, Mutex, String};
use crate::core::ObjectId;
use crate::platform::ime::{ImeBridge, ImeCandidatePosition, ImeComposition};

/// The Android input-method bridge.
#[derive(Debug)]
pub struct AndroidImeBridge {
    focused_widget: Mutex<Option<ObjectId>>,
    /// Whether the last `focus_in` successfully asked for the soft keyboard.
    soft_keyboard_shown: Mutex<bool>,
    marked_text: Mutex<String>,
    cursor_position: Mutex<usize>,
    composition: Mutex<ImeComposition>,
    candidate_position: Mutex<ImeCandidatePosition>,
}

impl Default for AndroidImeBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl AndroidImeBridge {
    /// Creates a bridge with no focused widget and no soft-keyboard request.
    pub fn new() -> Self {
        Self {
            focused_widget: Mutex::new(None),
            soft_keyboard_shown: Mutex::new(false),
            marked_text: Mutex::new(String::new()),
            cursor_position: Mutex::new(0),
            composition: Mutex::new(ImeComposition::default()),
            candidate_position: Mutex::new(ImeCandidatePosition::default()),
        }
    }

    /// The process-wide bridge.
    ///
    /// The bridge holds only process state (the focused widget and the composition preview), so
    /// one instance serves every window the Activity owns — the same shape the Windows and Linux
    /// bridges use.
    pub fn bridge() -> &'static AndroidImeBridge {
        static BRIDGE: std::sync::OnceLock<AndroidImeBridge> = std::sync::OnceLock::new();
        BRIDGE.get_or_init(AndroidImeBridge::new)
    }

    /// Clears the in-process composition without touching focus.
    fn clear_composition(&self) {
        *lock(&self.marked_text) = String::new();
        *lock(&self.cursor_position) = 0;
        *lock(&self.composition) = ImeComposition::default();
    }
}

impl ImeBridge for AndroidImeBridge {
    fn focus_in(&self, widget_id: ObjectId) {
        *lock(&self.focused_widget) = Some(widget_id);
        let shown = crate::platform::android_jni::show_soft_input();
        *lock(&self.soft_keyboard_shown) = shown;
        if !shown {
            log::warn!(
                "[Android IME] focus_in: the soft keyboard could not be shown (no Activity \
                 decor view, or the host has not attached a Context)"
            );
        }
        log::info!("[Android IME] focus_in: widget={widget_id} soft_keyboard={shown}");
    }

    fn focus_out(&self, widget_id: ObjectId) {
        *lock(&self.focused_widget) = None;
        self.clear_composition();
        let hidden = crate::platform::android_jni::hide_soft_input();
        *lock(&self.soft_keyboard_shown) = false;
        if !hidden {
            log::debug!(
                "[Android IME] focus_out: the soft keyboard was not visible or the host has no \
                 decor view"
            );
        }
        log::info!("[Android IME] focus_out: widget={widget_id} soft_keyboard_hidden={hidden}");
    }

    fn commit_text(&self, text: &str) {
        log::info!("[Android IME] commit_text: '{text}'");
        self.clear_composition();
        #[cfg(not(alloc_frugal))]
        {
            // Copy the focused id out before delivering: the widget's handler may change focus,
            // which re-enters this bridge's mutex. The Windows bridge documents the same rule.
            let focused = *lock(&self.focused_widget);
            if let Some(widget_id) = focused {
                if !crate::platform::ime::deliver_commit(widget_id, text) {
                    log::debug!(
                        "[Android IME] commit_text: widget {widget_id} is no longer mounted; the \
                         commit was not delivered"
                    );
                }
            }
        }
        #[cfg(alloc_frugal)]
        {
            let _ = text;
        }
    }

    fn set_composition(&self, composition: &ImeComposition) {
        let cursor = composition.cursor_position.min(composition.text.len());
        *lock(&self.marked_text) = composition.text.clone();
        *lock(&self.cursor_position) = cursor;
        *lock(&self.composition) = composition.clone();
    }

    fn set_candidate_window_position(&self, position: ImeCandidatePosition) {
        *lock(&self.candidate_position) = position;
    }

    /// Whether an input session is **actually controllable**.
    ///
    /// Focus alone is not enough: `show_soft_input` reports whether the `InputMethodManager`
    /// accepted the request. This is the same honest conjunction the Linux bridge uses (focus
    /// plus a live IBus connection) and the Windows bridge uses (focus plus a live TSF
    /// connection).
    fn is_active(&self) -> bool {
        lock(&self.focused_widget).is_some() && *lock(&self.soft_keyboard_shown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_out_clears_the_composition_and_the_keyboard_request() {
        let bridge = AndroidImeBridge::new();
        bridge.set_composition(&ImeComposition {
            text: "ni hao".to_string(),
            cursor_position: 6,
            selection_length: 0,
        });
        bridge.focus_in(7);
        // The host build has no JNI context, so `show_soft_input` answers false; `is_active`
        // must therefore stay false rather than claiming a keyboard it could not show.
        assert!(!bridge.is_active());
        bridge.focus_out(7);
        assert_eq!(*lock(&bridge.marked_text), "");
        assert!(!bridge.is_active());
    }

    #[test]
    fn a_commit_with_no_focus_is_harmless() {
        let bridge = AndroidImeBridge::new();
        bridge.commit_text("你好");
        assert_eq!(*lock(&bridge.marked_text), "");
    }
}
