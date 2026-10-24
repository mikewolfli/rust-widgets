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

/// One registry slot: the name, the outline, and the pixel density it was authored for.
///
/// `density` defaults to `1` for every registration made through the historical
/// [`register_icon_data`] entry point, so an existing host's table is unchanged. A second
/// registration under the same name with a higher density (for example `2` for a `@2x`
/// source) is stored in its own slot and selected when the widget's scale factor calls for it.
struct Slot {
    name: &'static str,
    data: IconData,
    density: u8,
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

#[cfg(not(test))]
static REGISTRY: Mutex<Registry> = Mutex::new(Registry::new());

// The registry a test build resolves against.
//
// A host registry is process-wide by design (register once, draw from every thread), which is
// wrong for a test binary: one test's registration would be visible to another's lookup and a
// `clear` in the first would empty the second's table mid-assertion. Per-thread storage keeps a
// test's icons its own. The value is leaked so the borrowed guard can live as long as the
// process-wide static it replaces; a test binary has a bounded number of threads.
#[cfg(test)]
thread_local! {
    static TEST_REGISTRY: &'static Mutex<Registry> =
        crate::compat::Box::leak(crate::compat::Box::new(Mutex::new(Registry::new())));
}

/// The registry this build stores into.
fn registry() -> &'static Mutex<Registry> {
    #[cfg(test)]
    {
        TEST_REGISTRY.with(|registry| *registry)
    }
    #[cfg(not(test))]
    {
        &REGISTRY
    }
}

/// Store `paths` under `name`, replacing an existing entry of the same name.
///
/// The validation (non-empty name/paths/grid) is the public module's job; this is the storage
/// operation, so it asserts only what it must to keep the table well-formed.
pub(crate) fn register_icon_data(
    name: &'static str,
    paths: &'static [&'static str],
    grid: u16,
) -> bool {
    register_icon_data_with_density(name, paths, grid, 1)
}

/// Store `paths` under `name` for a specific pixel `density`.
///
/// A density of `1` is the ordinary (unscaled) source and is what [`register_icon_data`]
/// stores. A higher density is an alternate source for the same name: the icon widget asks
/// for it when its configured scale factor reaches that density, so a host can register a
/// `@2x` outline next to the ordinary one without changing any existing registration.
///
/// Replacement is keyed by `(name, density)`, so re-registering one density leaves the other
/// densities of the same name in place. Densities below `1` are treated as `1`, because a
/// source cannot be less dense than the widget's logical grid.
pub(crate) fn register_icon_data_with_density(
    name: &'static str,
    paths: &'static [&'static str],
    grid: u16,
    density: u8,
) -> bool {
    let density = density.max(1);
    let data = IconData { name, grid, paths };
    let mut registry = crate::compat::lock(registry());
    let len = registry.len;
    // Replace in place, so a re-registration does not grow the table or reorder it.
    for slot in registry.slots[..len].iter_mut() {
        if let Some(existing) = slot {
            if existing.name == name && existing.density == density {
                *slot = Some(Slot { name, data, density });
                return true;
            }
        }
    }
    if len >= MAX_REGISTERED_ICONS {
        return false;
    }
    registry.slots[len] = Some(Slot { name, data, density });
    registry.len = len + 1;
    true
}

/// Remove every registered icon, returning how many there were.
pub(crate) fn clear_registered_icon_data() -> usize {
    let mut registry = crate::compat::lock(registry());
    let removed = registry.len;
    registry.slots = [const { None }; MAX_REGISTERED_ICONS];
    registry.len = 0;
    removed
}

/// How many icons are registered.
pub(crate) fn registered_icon_data_count() -> usize {
    crate::compat::lock(registry()).len
}

/// The outline registered under `name`, or `None`.
///
/// A linear scan of at most [`MAX_REGISTERED_ICONS`] entries under the lock. Chosen over a map:
/// sixteen string comparisons is faster than hashing a name, and the table fits in a cache line or
/// two, so a `BTreeMap`/`HashMap` would add allocation and code for a scan that is already trivial.
pub(crate) fn lookup_registered(name: &str) -> Option<IconData> {
    lookup_registered_scaled(name, 1.0)
}

/// The outline registered under `name` for `scale_factor`, or `None`.
///
/// At a scale factor of `2` or more the highest registered density of that name is
/// preferred, falling back to the ordinary density-`1` source and then to any source under
/// the name. Below `2` the density-`1` source is preferred. The fallback is deliberate: a
/// name registered only at `@2x` still draws at a normal scale rather than disappearing.
pub(crate) fn lookup_registered_scaled(name: &str, scale_factor: f32) -> Option<IconData> {
    let registry = crate::compat::lock(registry());
    let slots = registry.slots[..registry.len].iter().flatten();
    // Collect the best candidate in one pass without allocating: a linear scan over at most
    // `MAX_REGISTERED_ICONS` entries is what the registry was designed around.
    let mut best: Option<&Slot> = None;
    let high_density = scale_factor >= 2.0;
    for slot in slots {
        if slot.name != name {
            continue;
        }
        let wanted = if high_density { slot.density >= 2 } else { slot.density == 1 };
        if wanted && best.is_none_or(|current| slot.density > current.density) {
            best = Some(slot);
        }
    }
    if best.is_none() {
        // No source at the preferred density: accept the ordinary one (or any, as a last
        // resort) so a host that registered only one density never loses the icon.
        best = registry.slots[..registry.len]
            .iter()
            .flatten()
            .filter(|slot| slot.name == name)
            .min_by_key(|slot| slot.density);
    }
    best.map(|slot| slot.data)
}
