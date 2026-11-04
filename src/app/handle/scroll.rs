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
        crate::widget::runtime::read_widget(self.raw_id(), |widget| {
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
// Row selection – generic read/write pair (RW-122)
// ═══════════════════════════════════════════════════════════════

/// Writes a row container's selection through the property it publishes.
///
/// # Why a generic entry and not one property name
///
/// Row containers already publish their selection under three names (`current_row` on a list box,
/// `current_index` on tab-like containers, `selected_index` on tables and trees). This reads the
/// control's own capability table and writes the property it actually has, with the carrier its
/// declared kind asks for — a fourth spelling in the handle layer would be a parallel
/// implementation of the same fact (principle #101).
///
/// Only a control that also publishes `scroll_row` participates, so a tab widget's
/// `current_index` is not mistaken for a data row and a spin box's value is not a selection.
///
/// `false` means "this control has no writable selected-row property", which is distinct from a
/// refused write: the latter is logged by the property funnel.
pub(crate) fn write_selection_row(id: ObjectId, row: usize) -> bool {
    let Some((name, kind)) = selection_row_property(id) else {
        return false;
    };
    match selection_row_carrier(kind, row) {
        Some(value) => write_control_property(id, name, value),
        None => false,
    }
}

/// Reads a row container's selected row, whichever selected-row property it publishes.
pub(crate) fn read_selection_row(id: ObjectId) -> Option<usize> {
    let (name, _) = selection_row_property(id)?;
    match read_control_property(id, name) {
        Some(crate::widget::capability::CapabilityValue::UInt(value)) => {
            usize::try_from(value).ok()
        }
        Some(crate::widget::capability::CapabilityValue::Int(value)) => usize::try_from(value).ok(),
        Some(crate::widget::capability::CapabilityValue::Float(value)) if value >= 0.0 => {
            Some(value.round() as usize)
        }
        _ => None,
    }
}

/// The property name (and declared kind) a row container publishes its selection under.
///
/// The candidates are ordered from most specific to most general. A control that publishes both
/// `current_row` and `selected_index` (a list box) answers with `current_row`, which is its
/// authoritative selection; a table answers with `selected_index`.
fn selection_row_property(
    id: ObjectId,
) -> Option<(&'static str, crate::widget::capability::PropertyValueKind)> {
    crate::widget::runtime::read_widget(id, |widget| {
        let factory = crate::widget::capability::WidgetFactory::shared();
        let capability = factory.capability_for_kind_instance(widget)?;
        let publishes = |name: &str| {
            capability.properties.iter().any(|schema| schema.name == name && schema.readable)
        };
        if !publishes("scroll_row") {
            return None;
        }
        ["current_row", "selected_index", "current_index"].into_iter().find_map(|name| {
            capability
                .properties
                .iter()
                .find(|schema| schema.name == name && schema.readable && schema.writable)
                .map(|schema| (name, schema.value_kind))
        })
    })
    .flatten()
}

/// Builds the value a selected-row property's declared kind accepts.
fn selection_row_carrier(
    kind: crate::widget::capability::PropertyValueKind,
    row: usize,
) -> Option<crate::widget::capability::CapabilityValue> {
    use crate::widget::capability::PropertyValueKind as K;
    match kind {
        K::UInt => u64::try_from(row).ok().map(crate::widget::capability::CapabilityValue::UInt),
        K::Int => i64::try_from(row).ok().map(crate::widget::capability::CapabilityValue::Int),
        K::Float | K::Number => Some(crate::widget::capability::CapabilityValue::Float(row as f64)),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════
// TabWidgetHandle – content operations
// ═══════════════════════════════════════════════════════════════
