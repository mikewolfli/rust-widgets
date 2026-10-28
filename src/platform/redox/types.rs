// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Redox OS / Orbital backend state.
//!
//! # What this backend is for
//!
//! Redox OS has no GTK, no X11 and no Wayland: its display server is **Orbital**, reached through
//! the `/scheme/orbital` filesystem protocol. Winit's Orbital backend (`winit-orbital`) is the
//! reference for the protocol, and this module implements the same role for this crate's
//! self-drawn model: the host supplies a window and its pixel buffer, and the library paints every
//! control into it (`orbclient::Window` is the client half of that protocol).
//!
//! # What the host owns and what the library owns
//!
//! The backend owns one [`orbclient::Window`] per library window, its input state, and the scale
//! the compositor reports. The library owns every control, so a window's child tree is painted by
//! `widget::runtime::render_frame_tree_cached_shared` and blitted into `Window::data_mut` (skipped
//! entirely when the cache hands back the frame already on screen). No per-control native
//! creation exists here, exactly as on the other self-drawn hosts (rules #55/#56).
//!
//! # Threading
//!
//! Orbital is a filesystem protocol and its client window is `!Send` because it borrows a mapped
//! pixel buffer. The platform singleton requires `Send`, and every entry point that touches
//! Orbital runs on the thread that called [`crate::platform::Platform::init`], so the same
//! `unsafe impl Send` argument the GTK backend records applies: the type is moved between threads
//! only inside the process-global `OnceLock`, never used concurrently.

use crate::compat::{HashMap, Mutex};
use crate::core::ObjectId;
use crate::platform::state::BackendState;
use orbclient::Window;
use std::sync::atomic::{AtomicBool, AtomicU32};

/// Logical handle kinds this backend records.
///
/// Only windows are allocated here; every other control lives in `widget::runtime`, so a second
/// variant would name a host object this backend never creates.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RedoxHandleKind {
    /// A top-level Orbital window.
    Window,
}

/// Per-window input state that the protocol reports incrementally.
#[derive(Default)]
pub(crate) struct OrbitalInput {
    /// Modifier bitfield in the widget layer's convention (shift = 1, control = 2, alt = 4,
    /// meta = 8; see `event::Modifiers::from_event_bits`).
    pub(crate) modifiers: u32,
    /// Button state from the previous `ButtonEvent`, so a state change becomes a press/release.
    pub(crate) buttons: (bool, bool, bool),
    /// Whether the preceding `TextInput` event already delivered the printable character for the
    /// next key. Orbital emits `TextInput` before `Key`; forwarding both would insert the
    /// character twice.
    pub(crate) text_delivered: bool,
}

/// A live Orbital window and the state its event stream needs.
pub(crate) struct OrbitalWindow {
    /// The client window. Its buffer is the frame target.
    pub(crate) window: Window,
    /// Modifier/button state and the text/key pairing flag.
    pub(crate) input: OrbitalInput,
    /// The frame most recently converted and synced, kept so the next tick can tell "the cache
    /// handed back the same pixels" from "the content moved" without converting a whole frame
    /// (ENG-C1). Holding it also keeps its allocation alive, so the pointer comparison cannot be
    /// satisfied by a recycled allocation.
    pub(crate) last_frame: Option<crate::compat::Rc<Vec<u8>>>,
}

// SAFETY: `Window` is `!Send` because it holds a mapped pixel buffer. The whole per-window map is
// behind a `Mutex` and is only ever touched from the thread that drives the platform; the type is
// moved between threads solely because the process-global platform singleton must be `Send`.
unsafe impl Send for OrbitalWindow {}

/// The Redox/Orbital platform.
pub struct OrbitalPlatform {
    /// Window records and the accessors the library asks about them.
    pub(crate) state: BackendState<RedoxHandleKind>,
    /// Live windows, keyed by the platform id `create_window` returned.
    pub(crate) windows: Mutex<HashMap<ObjectId, OrbitalWindow>>,
    /// Whether the frame loop should keep running. `quit` may be called from another thread.
    pub(crate) running: AtomicBool,
    /// The compositor's scale factor as `f32` bits. Orbital reports it with `EVENT_SCALE`; the
    /// default is 1.0 (unscaled).
    pub(crate) scale_bits: AtomicU32,
    /// Screen size reported by Orbital, or `(0, 0)` before the first `EVENT_SCREEN`.
    pub(crate) screen: Mutex<(u32, u32)>,
}

impl OrbitalPlatform {
    /// Creates an empty backend. No Orbital window exists until `create_window`.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            windows: Mutex::new(HashMap::new()),
            running: AtomicBool::new(false),
            scale_bits: AtomicU32::new(1.0f32.to_bits()),
            screen: Mutex::new((0, 0)),
        }
    }
}

crate::impl_default_via_new!(OrbitalPlatform);
