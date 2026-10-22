// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared cleanup for the menu tables the non-Windows backends keep.
//!
//! # Why this exists (PLAT-08)
//!
//! Android, the mobile preview backend and Wayland each used to carry a hand-written copy of
//! "remove this widget from the attached-bar map, the menu tree and the trigger queue" inside
//! `destroy_widget`. The copies had drifted (one cleared only one direction of the attached-bar
//! map, one forgot the trigger queue), and a new side table had to be added to every copy.
//!
//! This module owns the rule once. Each backend's menu state exposes a `purge` method that
//! delegates here, so `destroy_widget` makes one call and the tables it owns are cleaned the same
//! way. Windows keeps a different model (native handle map + command-id map), so its `purge`
//! lives on its own state type but follows the same one-call shape.

use crate::compat::{HashMap, Vec, VecDeque};

/// Removes every reference to `widget_id` from the shared menu tables.
///
/// `pending_menu_events` is optional because the mobile preview backend keeps its trigger queue
/// in `BackendState::widget_events` rather than beside the menu model; passing `None` cleans the
/// tables it does own without inventing a second queue.
///
/// The `Ord` bound is not used by the standard `HashMap`; it is what lets this one body compile
/// where `crate::compat::HashMap` is a `BTreeMap` (the allocation-frugal profiles).
pub(crate) fn purge<Id>(
    attached_menu_bar: &mut HashMap<Id, Id>,
    menu_children: &mut HashMap<Id, Vec<Id>>,
    pending_menu_events: Option<&mut VecDeque<Id>>,
    widget_id: Id,
) where
    Id: Copy + Eq + core::hash::Hash + Ord,
{
    // Both directions: the widget may be an attached bar (keyed by the window) or a window
    // owning one (the value).
    attached_menu_bar.remove(&widget_id);
    attached_menu_bar.retain(|_, bar| *bar != widget_id);
    // It may be a container in the menu tree: drop the children it owned and the child entry
    // under its own parent.
    menu_children.remove(&widget_id);
    for children in menu_children.values_mut() {
        children.retain(|child| *child != widget_id);
    }
    // A trigger already queued for a widget that is going away would otherwise be drained and
    // delivered to whatever id is recycled next.
    if let Some(pending) = pending_menu_events {
        pending.retain(|queued| *queued != widget_id);
    }
}
