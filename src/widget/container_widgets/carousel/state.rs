// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `Carousel` session state (RW-64).
//!
//! The selected page is the only user-visible fact the carousel owns; the page list is the
//! host's model. Keeping the blob here lets the widget module stay focused on layout and paint.

use super::Carousel;
use crate::compat::Vec;
use crate::widget::save_state::SaveState;

/// Version tag of the blob produced by [`Carousel::save_state`].
const CAROUSEL_STATE_VERSION: u8 = 1;

impl Carousel {
    /// Serialises the selected page for session restore (RW-64).
    pub fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::with_capacity(5);
        state.push(CAROUSEL_STATE_VERSION);
        state.extend_from_slice(&(self.current_index as u32).to_le_bytes());
        state
    }

    /// Restores a blob produced by [`Self::save_state`].
    ///
    /// Returns `false` for a malformed blob or an empty carousel. The index is clamped to the
    /// current page set, because the pages are the host's model and a saved index is a
    /// preference rather than an assertion that the set did not shrink. The restore lands on
    /// the page directly rather than animating a slide from the current one.
    pub fn restore_state(&mut self, state: &[u8]) -> bool {
        if state.len() != 5 || state[0] != CAROUSEL_STATE_VERSION {
            return false;
        }
        if self.pages.is_empty() {
            return false;
        }
        let index = u32::from_le_bytes([state[1], state[2], state[3], state[4]]) as usize;
        let index = index.min(self.pages.len() - 1);
        self.current_index = index;
        self.slide = None;
        self.autoplay_elapsed_ms = 0;
        self.page_changed.emit(index);
        self.base.request_redraw();
        true
    }
}

/// RW-64: the selected page is the carousel's user-visible state.
impl SaveState for Carousel {
    fn state_version(&self) -> u16 {
        u16::from(CAROUSEL_STATE_VERSION)
    }

    fn save_state(&self) -> Vec<u8> {
        Carousel::save_state(self)
    }

    fn restore_state(&mut self, state: &[u8]) -> bool {
        Carousel::restore_state(self, state)
    }
}
