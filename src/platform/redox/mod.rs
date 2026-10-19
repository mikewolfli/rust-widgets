// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Redox OS backend over the Orbital display server.
//!
//! Redox has no GTK, X11 or Wayland; Orbital is its windowing protocol. The backend mirrors winit's
//! Orbital role without depending on winit: it opens a real `orbclient::Window`, paints the
//! library's control tree into its buffer, and routes Orbital's input records through the same
//! widget event entry points every other host uses.
//!
//! `mod.rs` holds only module wiring (rule #8): the state types live in [`types`] and the
//! `Platform` implementation in `platform_impl`.

mod platform_impl;

pub(crate) mod types;

pub use types::OrbitalPlatform;
