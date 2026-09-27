// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Playback state — re-exported from `core`, where it is defined.
//!
//! # Why this is a re-export and not a definition
//!
//! `PlaybackState` used to be defined here, inside the `video` feature. That put a plain five-variant
//! enum behind a codec dependency, so a control that only wanted to say "playing or paused" — the
//! `MediaPlayer` widget, which is always built — could not name it. The type moved to
//! [`crate::core::PlaybackState`] beside the clock that uses it, and every path that imported it from
//! here still works.
//!
//! Principle #54: a same-semantics type has exactly one definition, and the layer that re-exports it
//! refers to that definition rather than keeping a copy that can drift.

pub use crate::core::PlaybackState;
