// The inner module keeps the control's own name: the folder is the family and the file is
// the control, so the historical `…::media_player::media_player` path stays valid. `module_inception` would
// prefer a different inner name; renaming here would move a published module path for a lint.
#![allow(clippy::module_inception)]

// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Media player composite family.
//!
//! The family is the player itself plus the three controls a host hangs around it:
//!
//! - [`media_player`] — `MediaPlayer`, the player surface with its frame and repeat controls.
//! - [`media_transport`] — `MediaTransportBar`, the shared transport bar (play/pause, progress,
//!   volume, fullscreen, auto-hide).
//! - [`media_scrubber`] — `MediaScrubber`, the standalone draggable progress track.
//! - [`subtitle_bar`] — `SubtitleBar`, the caption overlay driven by a player's cue stream.
//!
//! Each file stays its own module; this folder only groups them. This `mod.rs` re-exports the
//! player's item surface at the folder root so `special_widgets::media_player::MediaPlayer` (and
//! the rest) keep resolving exactly as they did when the family was a single file in the parent
//! folder.

pub mod media_player;
pub mod media_scrubber;
pub mod media_transport;
pub mod subtitle_bar;

pub use media_player::{
    MediaFrame, MediaPlayer, RepeatMode, SubtitleCue, MAX_MEDIA_FRAME_DIMENSION,
};
