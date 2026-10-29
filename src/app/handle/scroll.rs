// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Scroll-event handles: state for the scroll-area and scroll-bar controls.

use super::*;
use crate::core::ObjectId;
use alloc::rc::Rc;
use core::cell::RefCell;
use std::collections::HashMap;

impl_handle!(ScrollAreaHandle, "Type-safe handle for a ScrollArea widget.");
impl_handle!(ScrollBarHandle, "Type-safe handle for a ScrollBar widget.");
// ═══════════════════════════════════════════════════════════════
// ScrollAreaHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
pub(crate) struct ScrollAreaState {
    scroll_x: i32,
    scroll_y: i32,
    content_w: u32,
    content_h: u32,
}

thread_local! {
    pub(crate) static SCROLL_AREA_STATES: RefCell<HashMap<ObjectId, ScrollAreaState>> = RefCell::new(HashMap::new());
}

/// # Scroll-area specific operations
impl ScrollAreaHandle {
    /// Set the scroll offset.
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well
    /// through [`crate::platform::Platform::set_widget_scroll_position`] (`SetScrollPos` on
    /// Windows, the GTK adjustments, the AppKit clip view).
    pub fn set_scroll_position(&self, x: i32, y: i32) {
        {
            let wrote_x = write_control_property(
                self.raw_id(),
                "scroll_position_x",
                crate::widget::capability::CapabilityValue::Int(i64::from(x)),
            );
            let wrote_y = write_control_property(
                self.raw_id(),
                "scroll_position_y",
                crate::widget::capability::CapabilityValue::Int(i64::from(y)),
            );
            note_property_pair("scroll_area.set_scroll_position", wrote_x, wrote_y);
        };
        // The control clamps the offset against its own content/viewport, so the mirror
        // records what the control **accepted**, not what the caller asked for. Reading it
        // back is what makes `scroll_position()` a fact rather than a request.
        let actual = |name, fallback| match read_control_property(self.raw_id(), name) {
            Some(crate::widget::capability::CapabilityValue::Int(value)) => value as i32,
            _ => fallback,
        };
        let actual_x = actual("scroll_position_x", x);
        let actual_y = actual("scroll_position_y", y);
        SCROLL_AREA_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.scroll_x = actual_x;
            state.scroll_y = actual_y;
        });
    }

    /// Return the current scroll offset.
    pub fn scroll_position(&self) -> (i32, i32) {
        SCROLL_AREA_STATES.with(|map| {
            let map = map.borrow();
            map.get(&self.raw_id()).map(|s| (s.scroll_x, s.scroll_y)).unwrap_or((0, 0))
        })
    }

    /// Set the content size (in virtual pixels).
    ///
    /// Written to the control through the `content_width`/`content_height` properties as
    /// well as recorded in the in-process mirror, so the control's own scroll clamp — and
    /// therefore [`Self::scroll_to_bottom`] — uses the size the caller declared. A previous
    /// version wrote only the mirror, which meant the control clamped every offset against
    /// its own (zero) content size and the scroll never moved.
    pub fn set_content_size(&self, w: u32, h: u32) {
        SCROLL_AREA_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.content_w = w;
            state.content_h = h;
        });
        let wrote_w = write_control_property(
            self.raw_id(),
            "content_width",
            crate::widget::capability::CapabilityValue::UInt(u64::from(w)),
        );
        let wrote_h = write_control_property(
            self.raw_id(),
            "content_height",
            crate::widget::capability::CapabilityValue::UInt(u64::from(h)),
        );
        note_property_pair("scroll_area.set_content_size", wrote_w, wrote_h);
        // The control clamps its current offset against the new extent. Refresh the mirror
        // through the same read-back path so a shrunken content size cannot leave a stale
        // scroll position reported to the caller.
        let (x, y) = self.scroll_position();
        self.set_scroll_position(x, y);
    }

    /// Returns the content size recorded for this scroll area.
    pub fn content_size(&self) -> (u32, u32) {
        SCROLL_AREA_STATES.with(|map| {
            let map = map.borrow();
            map.get(&self.raw_id()).map(|s| (s.content_w, s.content_h)).unwrap_or((0, 0))
        })
    }

    /// Sets or clears the scroll area's single content child.
    ///
    /// Returns `false` when the id is not a scroll area, or when a proposed child is not
    /// mounted. The control links both sides of the parent/child relation; a stale id would
    /// otherwise be visited by every tree walk with no widget behind it.
    pub fn set_content_widget(&self, content: Option<ObjectId>) -> bool {
        if let Some(id) = content {
            if !crate::widget::runtime::is_mounted(id) {
                return false;
            }
        }
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::ScrollArea>(widget)
                .map(|area| area.set_widget(content))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Returns the scroll area's content child, if any.
    pub fn content_widget(&self) -> Option<ObjectId> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::ScrollArea>(widget)
                .and_then(|area| area.widget())
        })
        .flatten()
    }

    /// Scroll to the bottom of the content.
    ///
    /// Computes the bottom offset from the last `set_content_size` height and pushes it through
    /// [`Self::set_scroll_position`], so the control scrolls rather than only the in-process
    /// mirror moving. The control clamps the offset against its own content size, which
    /// [`ScrollAreaHandle::set_content_size`] now writes through `content_width`/`content_height`;
    /// on a control that has no content the clamp still lands at 0.
    pub fn scroll_to_bottom(&self) {
        let bottom = SCROLL_AREA_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.content_h).unwrap_or(0));
        self.set_scroll_position(0, bottom as i32);
    }

    /// Scroll to the top of the content.
    pub fn scroll_to_top(&self) {
        self.set_scroll_position(0, 0);
    }
}

// ═══════════════════════════════════════════════════════════════
// TabWidgetHandle – content operations
// ═══════════════════════════════════════════════════════════════
