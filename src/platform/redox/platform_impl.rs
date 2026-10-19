// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `Platform` implementation for Redox OS over Orbital.
//!
//! # Scope, and the reference used
//!
//! Winit's `winit-orbital` backend is the protocol reference: Orbital is a filesystem scheme, a
//! window is a socket under `/scheme/orbital`, and input arrives as packed `orbclient::Event`
//! records. This module uses `orbclient`'s client window (the same crate winit's backend reads
//! its event types from) and keeps the crate's own self-drawn model: a window owns its pixel
//! buffer, and `widget::runtime::render_frame_tree` paints the whole control tree into it.
//!
//! # What is real here
//!
//! * A window is a real Orbital window; `create_window` opens it and `run` reads its event socket.
//! * Pointer, wheel, key and text input are translated into the library's [`Event`]s and routed
//!   through the same `dispatch_pointer_event` / `dispatch_event` entry points every backend uses.
//!   That means a click focuses the control it lands on on Redox exactly as it does elsewhere.
//! * Text arrives as `TextInput`, not as a key code: Orbital reports printable characters
//!   separately, and forwarding both would insert a character twice. A control chord suppresses
//!   the text so `Ctrl+C` remains a command.
//! * The clipboard is Orbital's own (`orbclient::Window::clipboard` / `set_clipboard`), with
//!   `MediaKind::Text` mapped to this crate's text accessors.
//! * Monitor size and scale are read from the compositor (`get_display_size`, `EVENT_SCALE`), so
//!   `displays()` reports one real screen instead of an invented one.
//!
//! # What is honest absence
//!
//! Orbital defines no input-method client, accessibility bridge, native menu protocol or system
//! file picker. `capabilities()` reports those as `false`, and the trait defaults answer
//! `Unsupported`/`None` rather than a fabricated success. Winit reaches the same conclusions
//! (`ime_capabilities() -> None`, no monitors, no menus).
//!
//! # Verification boundary
//!
//! A Redox target has no prebuilt `std`, and building one here fails in the standard library's
//! dependency graph before this crate is reached. The backend is therefore **not compiled or run
//! on this host**; `tools/check_redox_cross.sh` reports that as a host limitation (exit 2) and the
//! log records it as an open verification item rather than as a passing gate (principle #103).

use crate::compat::{String, Vec};
use crate::core::{ObjectId, PlatformFamily, Point, Rect, Size};
use crate::event::Event;
use crate::platform::redox::types::{OrbitalPlatform, OrbitalWindow, RedoxHandleKind};
use crate::platform::types::{Display, DisplayId, OpenError, Platform, PlatformCapabilities};
use crate::widget::runtime;
use core::time::Duration;
use orbclient::{
    get_display_size, Color as OrbitalColor, EventOption, MediaKind, Renderer, Window, WindowFlag,
    WindowFlags,
};
use std::sync::atomic::Ordering;
use std::thread;

/// The frame interval this backend advances the library by, in milliseconds.
///
/// One constant, so the number handed to [`crate::drive_frame`] cannot differ from the other
/// hosts' — a transition must take the time the theme said it should.
const FRAME_INTERVAL_MS: u32 = 16;

/// Widget-layer modifier bits (see `event::Modifiers::from_event_bits`).
const MOD_SHIFT: u32 = 1;
const MOD_CONTROL: u32 = 2;
const MOD_ALT: u32 = 4;
const MOD_META: u32 = 8;

/// Work collected from a window's event socket and applied on the library thread outside the
/// window lock, so a callback may call back into the platform without deadlocking.
enum Pending {
    /// A pointer event in window space.
    Pointer(ObjectId, Event, Point),
    /// A key/text event for a window's focused control.
    Key(ObjectId, Event),
    /// Tab traversal, a library action rather than content.
    Tab(bool),
    /// A container reported a new client size.
    Resize(ObjectId, u32, u32),
    /// The window lost the keyboard.
    FocusLost,
    /// The pointer left every control.
    Leave(Point),
    /// The compositor asked the window to close.
    Close(ObjectId, ObjectId),
}

impl Platform for OrbitalPlatform {
    // The uniform widget-property methods are answered once, over `self.state`, by the shared
    // expansion rather than re-written here.
    crate::impl_platform_state_properties!();

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "redox-orbital"
    }

    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    /// Orbital has no `/proc` in the Linux format this crate probes, so every OS figure is an
    /// honest `None`/`false` rather than a made-up number.
    fn total_memory_mb(&self) -> Option<u64> {
        None
    }

    fn is_on_battery(&self) -> bool {
        false
    }

    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            // `EVENT_SCALE` is real and this backend keeps it; a window's logical geometry is
            // therefore scaled by a value the compositor reported.
            dpi_scaling: true,
            // Orbital defines no input-method client (winit's backend also reports `None`).
            ime: false,
            accessibility: false,
            native_menu: false,
            // The shared `BackendState` queue, backed by the same drain the other hosts use.
            typed_widget_trigger: true,
            // Orbital's window scheme carries a real text clipboard.
            clipboard: true,
            open_external: false,
            file_dialog: false,
        }
    }

    fn init(&self) {
        if let Ok((width, height)) = get_display_size() {
            *crate::compat::lock(&self.screen) = (width, height);
        }
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let id = self.state.create_widget(RedoxHandleKind::Window, title, x, y, width, height);
        // `Async` makes `Window::events` return what is queued instead of blocking, which is what
        // lets the frame tick keep painting and advancing the animation bus. `Resizable` and
        // `Scalable` are the two flags Orbital defines for a normal application window.
        let flags =
            WindowFlags::new(&[WindowFlag::Async, WindowFlag::Resizable, WindowFlag::Scalable]);
        match Window::new_flags(x, y, width, height, title, flags) {
            Some(window) => {
                if let Ok((w, h)) = get_display_size() {
                    *crate::compat::lock(&self.screen) = (w, h);
                }
                crate::compat::lock(&self.windows)
                    .insert(id, OrbitalWindow { window, input: Default::default() });
                id
            }
            None => {
                // No Orbital display (or the scheme refused the request). Reporting `0` is the
                // documented "no window was created" answer; the state record must not survive it.
                self.state.destroy_widget(id);
                log::error!(
                    "[redox] create_window: Orbital refused a {width}x{height} window titled \
                     {title:?}; is the Orbital display running?"
                );
                0
            }
        }
    }

    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        let removed = crate::compat::lock(&self.windows).remove(&widget_id).is_some();
        // Dropping the `Window` closes its Orbital socket.
        removed || self.state.destroy_widget(widget_id)
    }

    fn window_client_size(&self, window_id: ObjectId) -> Option<(u32, u32)> {
        let host = if crate::compat::lock(&self.windows).contains_key(&window_id) {
            Some(window_id)
        } else {
            runtime::host_window_for(window_id)
        };
        let windows = crate::compat::lock(&self.windows);
        host.and_then(|host| windows.get(&host))
            .map(|entry| (entry.window.width().max(1), entry.window.height().max(1)))
            .or_else(|| self.state.window_size(window_id))
    }

    fn queue_resize_trigger(&self, window_id: ObjectId, width: u32, height: u32) -> bool {
        // Forward to the control backend, which owns the queue the host polls. Writing to this
        // backend's state would land in a store the trigger drain never reads.
        crate::queue_resize_trigger(window_id, width, height)
    }

    fn displays(&self) -> Vec<Display> {
        let (width, height) = *crate::compat::lock(&self.screen);
        if width == 0 || height == 0 {
            return Vec::new();
        }
        let scale = self.dpi_scale_factor();
        let logical_width = (width as f32 / scale).round().max(1.0) as u32;
        let logical_height = (height as f32 / scale).round().max(1.0) as u32;
        vec![Display {
            id: DisplayId::new(1),
            bounds: Rect::new(0, 0, logical_width, logical_height),
            work_area: Rect::new(0, 0, logical_width, logical_height),
            scale,
            is_primary: true,
        }]
    }

    fn display_of_window(&self, window_id: ObjectId) -> Option<DisplayId> {
        if crate::compat::lock(&self.windows).contains_key(&window_id)
            || runtime::host_window_for(window_id)
                .is_some_and(|host| crate::compat::lock(&self.windows).contains_key(&host))
        {
            Some(DisplayId::new(1))
        } else {
            None
        }
    }

    fn display_generation(&self) -> u64 {
        let (width, height) = *crate::compat::lock(&self.screen);
        if width == 0 || height == 0 {
            // No screen has been reported yet: "this backend cannot observe a change".
            0
        } else {
            1
        }
    }

    fn dpi_scale_factor(&self) -> f32 {
        let scale = f32::from_bits(self.scale_bits.load(Ordering::Relaxed));
        if scale.is_finite() && scale >= 1.0 {
            scale
        } else {
            1.0
        }
    }

    fn run(&self) {
        if !self.running.load(Ordering::SeqCst) {
            self.init();
            self.running.store(true, Ordering::SeqCst);
        }
        while self.running.load(Ordering::SeqCst) {
            self.pump();
            crate::drive_frame(FRAME_INTERVAL_MS);
            thread::sleep(Duration::from_millis(FRAME_INTERVAL_MS as u64));
        }
    }

    fn quit(&self) -> bool {
        self.running.store(false, Ordering::SeqCst);
        true
    }

    fn set_clipboard_text(&self, text: &str) -> bool {
        let mut windows = crate::compat::lock(&self.windows);
        match windows.values_mut().next() {
            Some(entry) => entry.window.set_clipboard(text, MediaKind::Text),
            None => false,
        }
    }

    fn get_clipboard_text(&self) -> String {
        self.clipboard_text().unwrap_or_default()
    }

    fn clipboard_text(&self) -> Option<String> {
        let windows = crate::compat::lock(&self.windows);
        match windows.values().next()?.window.clipboard()? {
            (MediaKind::Text, text) => Some(text),
            _ => None,
        }
    }

    fn open_external(&self, _target: &str) -> Result<(), OpenError> {
        Err(OpenError::Unsupported)
    }

    fn has_external_handler(&self) -> bool {
        false
    }

    /// Pops the next typed widget trigger from the shared queue.
    fn poll_widget_trigger_event(&self) -> Option<crate::platform::WidgetTriggerEvent> {
        self.state.pop_widget_trigger_event()
    }

    /// Pushes a typed widget trigger, the same shared queue the other hosts use.
    fn inject_widget_trigger_event(
        &self,
        widget_id: ObjectId,
        kind: crate::platform::WidgetTriggerKind,
    ) -> bool {
        self.state.inject_widget_trigger_event(widget_id, kind)
    }

    /// The id-only view of the same queue.
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }
}

impl OrbitalPlatform {
    /// Drains every live window's event socket, dispatches the translated events and paints one
    /// frame per window.
    ///
    /// # Why events are collected and dispatched outside the lock
    ///
    /// `Window::events` borrows the window mutably, but a dispatched event can call a host
    /// callback that asks this same platform for the clipboard or a window size. Holding the
    /// window map across that call would deadlock; the events are therefore copied out first and
    /// the lock is released before dispatch.
    fn pump(&self) {
        let mut pending = Vec::new();
        {
            let mut windows = crate::compat::lock(&self.windows);
            for (&host, entry) in windows.iter_mut() {
                let Some(window_widget) = runtime::widget_id_for_host_window(host) else {
                    continue;
                };
                let events: Vec<EventOption> =
                    entry.window.events().map(|event| event.to_option()).collect();
                for option in events {
                    self.translate(host, window_widget, entry, option, &mut pending);
                }
            }
        }
        self.dispatch(pending);
        self.paint();
    }

    /// Translates one Orbital event into zero or more library events.
    fn translate(
        &self,
        host: ObjectId,
        window_widget: ObjectId,
        entry: &mut OrbitalWindow,
        option: EventOption,
        pending: &mut Vec<Pending>,
    ) {
        match option {
            EventOption::TextInput(text) => {
                // A control chord is a command, not content. Orbital reports the printable
                // character even while Control is held, so the modifier state is the only place
                // that can tell `Ctrl+C` from a typed `c`.
                if entry.input.modifiers & (MOD_CONTROL | MOD_ALT | MOD_META) == 0 {
                    pending.push(Pending::Key(
                        window_widget,
                        Event::TextInput { text: text.character.to_string() },
                    ));
                    entry.input.text_delivered = true;
                }
            }
            EventOption::Key(key) => {
                match key.scancode {
                    orbclient::K_LEFT_SHIFT | orbclient::K_RIGHT_SHIFT => {
                        entry.input.modifiers =
                            set_modifier(entry.input.modifiers, MOD_SHIFT, key.pressed);
                    }
                    orbclient::K_CTRL | orbclient::K_RIGHT_CTRL => {
                        entry.input.modifiers =
                            set_modifier(entry.input.modifiers, MOD_CONTROL, key.pressed);
                    }
                    // `K_RIGHT_CTRL` and `K_ALT_GR` share one scancode in the Orbital protocol, so the
                    // control arm above owns it. Left Alt is still distinguishable and is handled here.
                    orbclient::K_ALT => {
                        entry.input.modifiers =
                            set_modifier(entry.input.modifiers, MOD_ALT, key.pressed);
                    }
                    orbclient::K_LEFT_SUPER | orbclient::K_RIGHT_SUPER => {
                        entry.input.modifiers =
                            set_modifier(entry.input.modifiers, MOD_META, key.pressed);
                    }
                    _ => {}
                }
                if !key.pressed {
                    entry.input.text_delivered = false;
                    if let Some(code) = scancode_to_widget_key(key.scancode) {
                        pending.push(Pending::Key(
                            window_widget,
                            Event::KeyRelease { key: code, modifiers: entry.input.modifiers },
                        ));
                    }
                    return;
                }
                if entry.input.text_delivered {
                    // The `TextInput` before this key already delivered the character.
                    entry.input.text_delivered = false;
                    return;
                }
                if key.scancode == orbclient::K_TAB {
                    pending.push(Pending::Tab(entry.input.modifiers & MOD_SHIFT == 0));
                    return;
                }
                if let Some(code) = scancode_to_widget_key(key.scancode) {
                    pending.push(Pending::Key(
                        window_widget,
                        Event::KeyPress { key: code, modifiers: entry.input.modifiers },
                    ));
                }
            }
            EventOption::Mouse(mouse) => {
                let point = Point::new(mouse.x, mouse.y);
                pending.push(Pending::Pointer(
                    window_widget,
                    Event::MouseMove { pos: point },
                    point,
                ));
            }
            EventOption::Button(button) => {
                // Orbital reports the **state** of all three buttons; a change in one of them is
                // the press or release the library consumes.
                let previous =
                    [entry.input.buttons.0, entry.input.buttons.1, entry.input.buttons.2];
                let current = [button.left, button.middle, button.right];
                for (index, (was, is)) in previous.into_iter().zip(current.into_iter()).enumerate()
                {
                    if was == is {
                        continue;
                    }
                    let event = if is {
                        Event::mouse_press_with(0, 0, index as u32 + 1, entry.input.modifiers)
                    } else {
                        Event::MouseRelease { pos: Point::new(0, 0), button: index as u32 + 1 }
                    };
                    pending.push(Pending::Key(window_widget, event));
                }
                entry.input.buttons = (current[0], current[1], current[2]);
            }
            EventOption::Scroll(scroll) => {
                pending.push(Pending::Key(
                    window_widget,
                    Event::Wheel {
                        delta: Point::new(scroll.x, scroll.y),
                        modifiers: entry.input.modifiers,
                    },
                ));
            }
            EventOption::Focus(focus) => {
                if !focus.focused {
                    pending.push(Pending::FocusLost);
                }
            }
            EventOption::Hover(hover) => {
                if !hover.entered {
                    pending.push(Pending::Leave(Point::new(0, 0)));
                }
            }
            EventOption::Resize(resize) => {
                pending.push(Pending::Resize(window_widget, resize.width, resize.height));
            }
            EventOption::Scale(scale) => {
                if scale.baseline > 0 && scale.scale > 0 {
                    let value = scale.scale as f32 / scale.baseline as f32;
                    self.scale_bits.store(value.to_bits(), Ordering::Relaxed);
                }
            }
            EventOption::Screen(screen) => {
                *crate::compat::lock(&self.screen) = (screen.width, screen.height);
            }
            EventOption::Quit(_) => {
                pending.push(Pending::Close(host, window_widget));
            }
            _ => {}
        }
    }

    /// Applies the collected work on the library thread.
    fn dispatch(&self, pending: Vec<Pending>) {
        let mut closes = Vec::new();
        for item in pending {
            match item {
                Pending::Pointer(root, event, point) => {
                    runtime::dispatch_pointer_event(root, &event, point);
                }
                Pending::Key(root, event) => {
                    let target = runtime::focused_widget().unwrap_or(root);
                    runtime::dispatch_event(target, &event);
                }
                Pending::Tab(forward) => {
                    runtime::focus_next(forward);
                }
                Pending::Resize(window, width, height) => {
                    self.queue_resize_trigger(window, width, height);
                }
                Pending::FocusLost => {
                    runtime::clear_focus();
                }
                Pending::Leave(point) => {
                    runtime::clear_hover(point);
                }
                Pending::Close(host, window) => closes.push((host, window)),
            }
        }
        for (host, window) in closes {
            crate::close_widget(window);
            runtime::unregister(window);
            self.destroy_widget(host);
        }
    }

    /// Paints and presents one frame per window.
    fn paint(&self) {
        let mut windows = crate::compat::lock(&self.windows);
        for (&host, entry) in windows.iter_mut() {
            let Some(window_widget) = runtime::widget_id_for_host_window(host) else {
                continue;
            };
            // Mirror the model's title and geometry onto the OS window. The state record is the
            // authority the library writes; Orbital only knows what it was told.
            let title = self.state.text(host);
            if entry.window.title() != title {
                entry.window.set_title(&title);
            }
            if let Some((x, y, width, height)) = self.state.widget_geometry(host) {
                if entry.window.x() != x || entry.window.y() != y {
                    entry.window.set_pos(x, y);
                }
                if entry.window.width() != width || entry.window.height() != height {
                    entry.window.set_size(width, height);
                }
            }
            let width = entry.window.width().max(1);
            let height = entry.window.height().max(1);
            let Some(frame) = runtime::render_frame_tree(
                window_widget,
                Size::new(width, height),
                crate::core::Color::rgb(240, 240, 240),
            ) else {
                continue;
            };
            {
                let pixels = entry.window.data_mut();
                for (pixel, rgba) in pixels.iter_mut().zip(frame.chunks_exact(4)) {
                    *pixel = OrbitalColor::rgba(rgba[0], rgba[1], rgba[2], rgba[3]);
                }
            }
            entry.window.sync();
        }
    }
}

/// Sets or clears one modifier bit.
fn set_modifier(current: u32, bit: u32, pressed: bool) -> u32 {
    if pressed {
        current | bit
    } else {
        current & !bit
    }
}

/// Translates an Orbital scancode into the key code the widget layer consumes.
///
/// The widget layer's convention is the X11-style keysym for printable keys and the ASCII control
/// code for navigation keys (Backspace 8, Tab 9, Enter 13, Escape 27, arrows 37–40 …). Only keys
/// with a defined mapping are forwarded; an unmapped media key is left to the host rather than
/// delivered as a number no control can interpret.
fn scancode_to_widget_key(scancode: u8) -> Option<u32> {
    use orbclient::*;
    let code = match scancode {
        // Letters. The key code is the uppercase ASCII value, which is what the widget layer's
        // shortcut comparison uses (`Ctrl+C` is key 67).
        K_A => 'A' as u32,
        K_B => 'B' as u32,
        K_C => 'C' as u32,
        K_D => 'D' as u32,
        K_E => 'E' as u32,
        K_F => 'F' as u32,
        K_G => 'G' as u32,
        K_H => 'H' as u32,
        K_I => 'I' as u32,
        K_J => 'J' as u32,
        K_K => 'K' as u32,
        K_L => 'L' as u32,
        K_M => 'M' as u32,
        K_N => 'N' as u32,
        K_O => 'O' as u32,
        K_P => 'P' as u32,
        K_Q => 'Q' as u32,
        K_R => 'R' as u32,
        K_S => 'S' as u32,
        K_T => 'T' as u32,
        K_U => 'U' as u32,
        K_V => 'V' as u32,
        K_W => 'W' as u32,
        K_X => 'X' as u32,
        K_Y => 'Y' as u32,
        K_Z => 'Z' as u32,
        // Digits.
        K_0 => '0' as u32,
        K_1 => '1' as u32,
        K_2 => '2' as u32,
        K_3 => '3' as u32,
        K_4 => '4' as u32,
        K_5 => '5' as u32,
        K_6 => '6' as u32,
        K_7 => '7' as u32,
        K_8 => '8' as u32,
        K_9 => '9' as u32,
        // Punctuation.
        K_MINUS => '-' as u32,
        K_EQUALS => '=' as u32,
        K_BRACE_OPEN => '[' as u32,
        K_BRACE_CLOSE => ']' as u32,
        K_SEMICOLON => ';' as u32,
        K_QUOTE => '\'' as u32,
        K_TICK => '`' as u32,
        K_BACKSLASH => '\\' as u32,
        K_COMMA => ',' as u32,
        K_PERIOD => '.' as u32,
        K_SLASH => '/' as u32,
        // Control and navigation keys, using the ASCII codes the widget layer already handles.
        K_ESC => 27,
        K_BKSP => 8,
        K_TAB => 9,
        K_ENTER => 13,
        K_SPACE => 32,
        K_INSERT => 45,
        K_DEL => 46,
        K_HOME => 36,
        K_END => 35,
        K_PGUP => 33,
        K_PGDN => 34,
        K_LEFT => 37,
        K_UP => 38,
        K_RIGHT => 39,
        K_DOWN => 40,
        K_F1 => 112,
        K_F2 => 113,
        K_F3 => 114,
        K_F4 => 115,
        K_F5 => 116,
        K_F6 => 117,
        K_F7 => 118,
        K_F8 => 119,
        K_F9 => 120,
        K_F10 => 121,
        K_F11 => 122,
        K_F12 => 123,
        _ => return None,
    };
    Some(code)
}
