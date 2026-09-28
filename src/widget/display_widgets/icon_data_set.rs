// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The storage behind the runtime icon registry (`icon_registry`).
//!
//! # Why the storage is its own module
//!
//! The public surface (`register_icon`, `clear_registered_icons`, …) is in `icon_registry`, which
//! is about *what a host can do*. This module is about *how it is stored*: a fixed-capacity table
//! behind a mutex, read on the draw path. Keeping them apart means the storage can be reasoned
//! about (and its concurrency argued) without the public API in view, and the public module can
//! change its shape without touching the lock.
//!
//! # Why a plain mutex, given the draw path
//!
//! A registered icon is looked up by name once per `Icon::draw`. The crate's constraints forbid
//! shared mutable state on the draw path, and this is a narrow, deliberate exception — the same one
//! the font registry makes: the state is a small table whose entries are `Copy`, the lock is held
//! for a name comparison and a copy-out, and it is never held across rasterisation. The alternative
//! (a lock-free table) would be more code for a lookup that happens once per widget per frame.
//!
//! # Why this is compiled whether or not `icons` is on
//!
//! A host icon is path data the host supplies; it does not depend on the crate's bundled table. A
//! build without `icons` still draws the fallback for its own tokens and *should* still draw a
//! host's registered outline — gating this on `icons` would make the registry a feature of a
//! feature, which is not what a host asked for.

use crate::compat::Mutex;
use crate::widget::display_widgets::icon::IconData;
use crate::widget::icon_registry::MAX_REGISTERED_ICONS;

/// One registry slot: the name, and the outline registered under it.
struct Slot {
    name: &'static str,
    data: IconData,
}

/// The registry table and how many slots are occupied.
struct Registry {
    slots: [Option<Slot>; MAX_REGISTERED_ICONS],
    len: usize,
}

impl Registry {
    /// An empty registry. `const` so it can initialise the static without a lazy wrapper.
    const fn new() -> Self {
        // `[None; N]` is not available for a non-`Copy` `Option<Slot>`, so the array is built by
        // a const block: `Slot` holds `&'static` values and has no `Drop`, so this is a plain
        // zeroing-equivalent.
        Self { slots: [const { None }; MAX_REGISTERED_ICONS], len: 0 }
    }
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry::new());

/// Store `paths` under `name`, replacing an existing entry of the same name.
///
/// The validation (non-empty name/paths/grid) is the public module's job; this is the storage
/// operation, so it asserts only what it must to keep the table well-formed.
pub(crate) fn register_icon_data(
    name: &'static str,
    paths: &'static [&'static str],
    grid: u16,
) -> bool {
    let data = IconData { name, grid, paths };
    let mut registry = crate::compat::lock(&REGISTRY);
    let len = registry.len;
    // Replace in place, so a re-registration does not grow the table or reorder it.
    for slot in registry.slots[..len].iter_mut() {
        if let Some(existing) = slot {
            if existing.name == name {
                *slot = Some(Slot { name, data });
                return true;
            }
        }
    }
    if len >= MAX_REGISTERED_ICONS {
        return false;
    }
    registry.slots[len] = Some(Slot { name, data });
    registry.len = len + 1;
    true
}

/// Remove every registered icon, returning how many there were.
pub(crate) fn clear_registered_icon_data() -> usize {
    let mut registry = crate::compat::lock(&REGISTRY);
    let removed = registry.len;
    registry.slots = [const { None }; MAX_REGISTERED_ICONS];
    registry.len = 0;
    removed
}

/// How many icons are registered.
pub(crate) fn registered_icon_data_count() -> usize {
    crate::compat::lock(&REGISTRY).len
}

/// The outline registered under `name`, or `None`.
///
/// A linear scan of at most [`MAX_REGISTERED_ICONS`] entries under the lock. Chosen over a map:
/// sixteen string comparisons is faster than hashing a name, and the table fits in a cache line or
/// two, so a `BTreeMap`/`HashMap` would add allocation and code for a scan that is already trivial.
pub(crate) fn lookup_registered(name: &str) -> Option<IconData> {
    let registry = crate::compat::lock(&REGISTRY);
    registry.slots[..registry.len]
        .iter()
        .flatten()
        .find(|slot| slot.name == name)
        .map(|slot| slot.data)
}
