// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Host-registered icons: a name and its outline, added to the built-in set at runtime.
//!
//! # The gap this closes
//!
//! [`IconName`](crate::widget::IconName) is a closed enum generated from
//! `tools/icon_tokens.txt` — a *fixed* set. That is the right shape for the crate's own icons
//! (a compile-time table, no lookup cost, no missing-icon surprise), but it leaves a host with no
//! way to draw **its own** icon: an application-specific glyph, a brand mark, a product icon set
//! the crate does not ship.
//!
//! This module is that way in. [`register_icon`] binds a token to SVG path data, and
//! [`Icon::set_icon`](crate::widget::Icon::set_icon) — which already accepts any string — draws the
//! registered outline for a name no `IconName` variant covers.
//!
//! # Why this is separate from `IconName`
//!
//! The two answer different questions and must not be conflated:
//!
//! * **`IconName`** is the crate's *published vocabulary*. A variant existing is a promise that the
//!   icon ships and is tested (the census covers every variant). It must stay a closed set, or that
//!   promise has no meaning.
//! * **The registry** is the *host's extension point*. It makes no promise about what is
//!   registered, only about how it is looked up and drawn.
//!
//! Folding host icons into `IconName` would need a runtime-sized enum, which is what the crate's
//! zero-cost-abstraction rule (principle #28) exists to avoid — and would make `ALL` unanswerable.
//!
//! # Why this is worth having when the icon set is already 68 deep
//!
//! `docs/plans/research-icons.md` §2 rejects an *icon font* partly because a glyph cannot be cached
//! the way static geometry can. That reasoning applies to the crate's **own** set (which is why it
//! is bundled path data) and not to this: a registered icon carries the same `&'static str` path
//! data as a bundled one, so it goes through the identical draw path. What the registry adds is
//! *reach* — a host's own icons — for the cost of one small table.

use crate::widget::display_widgets::icon_data_set::{lookup_registered, register_icon_data};

/// How many host icons may be registered at once.
///
/// A fixed capacity, like the runtime font registry: the lookup runs per draw, from threads that
/// hold no lock, and a growable table behind a lock would put that lock on the draw path. Sixteen
/// is a UI's worth of application-specific glyphs (a title bar and a toolbar) without making the
/// per-lookup copy large.
pub const MAX_REGISTERED_ICONS: usize = 16;

/// Register `paths` as the outline for `name`, replacing any earlier registration of that name.
///
/// Returns `true` when the icon was accepted, `false` when the registry is full or an argument is
/// empty. A `false` is a refusal, not a silent drop: a host that registered an icon and got `false`
/// must not believe its icon is drawable.
///
/// # The path data
///
/// `paths` is SVG path data (`d`) on the same 960-unit design grid as the built-in icons, with
/// **negative `y` upward** — see [`IconData`](crate::widget::IconData). A host can take that
/// straight from a Material Symbols file, or from any source that uses that grid; a different grid
/// will draw at the wrong scale, because the grid is a property of the data and this signature
/// does not carry it. `register_icon_on_grid` exists for a source on another grid.
///
/// # Why the name is `&'static str`
///
/// The same reason the font registry takes `&'static [u8]`: the lookup runs per draw and must not
/// allocate or lock. A `String` would force one or the other. A host that has a runtime name can
/// `Box::leak` it — an explicit, one-time cost.
///
/// # Example
///
/// ```no_run
/// use rust_widgets::widget::register_icon;
///
/// // A 960-grid outline with negative y upward, as Material Symbols files use.
/// let triangle = ["M480-200 240-440l56-56 184 184 184-184 56 56-240 240Z"];
/// assert!(register_icon("disclosure", &triangle));
/// ```
pub fn register_icon(name: &'static str, paths: &'static [&'static str]) -> bool {
    register_icon_on_grid(name, paths, 960)
}

/// [`register_icon`], for a source on a grid other than the built-in 960 units.
///
/// A host that draws from a 24-unit viewBox (Lucide, Tabler, Feather) passes `24` here rather than
/// rescaling the path, so the data stays the source's own numbers.
pub fn register_icon_on_grid(
    name: &'static str,
    paths: &'static [&'static str],
    grid: u16,
) -> bool {
    if name.is_empty() || paths.is_empty() || grid == 0 {
        return false;
    }
    // Every path must have something in it: an empty `d` draws nothing, which is
    // indistinguishable on screen from a missing icon, so it is refused rather than stored.
    if paths.iter().any(|d| d.trim().is_empty()) {
        return false;
    }
    register_icon_data(name, paths, grid)
}

/// Forget every registered icon. The built-in set is unaffected.
///
/// Returns how many were removed, so the effect is observable rather than something a test has to
/// infer.
pub fn clear_registered_icons() -> usize {
    crate::widget::display_widgets::icon_data_set::clear_registered_icon_data()
}

/// How many host icons are currently registered.
pub fn registered_icon_count() -> usize {
    crate::widget::display_widgets::icon_data_set::registered_icon_data_count()
}

/// Whether `name` is a registered host icon (as opposed to a built-in [`IconName`] token).
///
/// A caller uses this to tell "my icon is registered" from "the crate happens to ship this name",
/// which matters when deciding whether a missing icon is a registration bug.
///
/// [`IconName`]: crate::widget::IconName
pub fn is_registered_icon(name: &str) -> bool {
    lookup_registered(name).is_some()
}

/// A registered icon's outline, or `None` when the name is not registered.
///
/// Exposed so a test can assert what was stored without rendering, and so a host can check its own
/// registration without a draw.
pub fn registered_icon(name: &str) -> Option<RegisteredIcon> {
    lookup_registered(name).map(|data| RegisteredIcon {
        name: data.name,
        grid: data.grid,
        paths: data.paths,
    })
}

/// A host-registered icon, as [`registered_icon`] returns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisteredIcon {
    /// The token the host registered.
    pub name: &'static str,
    /// The design grid its path data is on.
    pub grid: u16,
    /// The SVG path data, one entry per `<path>`.
    pub paths: &'static [&'static str],
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry is process-global, so one test owns the whole sequence to avoid interleaving.
    #[test]
    fn registration_refuses_junk_and_honours_capacity_and_replacement() {
        clear_registered_icons();
        assert_eq!(registered_icon_count(), 0);

        // Junk is refused: an empty name, empty paths, an empty `d`, a zero grid.
        assert!(!register_icon("", &["M0 0Z"]));
        assert!(!register_icon("empty", &[]));
        assert!(!register_icon("blank", &["   "]));
        assert!(!register_icon_on_grid("zerogrid", &["M0 0Z"], 0));
        assert_eq!(registered_icon_count(), 0, "a refused icon must not occupy a slot");

        assert!(register_icon("mine", &["M0 0L10 0L10 10Z"]));
        assert!(is_registered_icon("mine"));
        assert!(registered_icon("mine").is_some());
        assert_eq!(registered_icon("mine").map(|icon| icon.grid), Some(960));

        // Replacement, not duplication: the same name twice is one entry with the newer data.
        assert!(register_icon_on_grid("mine", &["M0 0L20 0L20 20Z"], 24));
        assert_eq!(registered_icon_count(), 1, "a same-named registration replaces");
        let stored = registered_icon("mine").expect("registered");
        assert_eq!(stored.grid, 24, "the replacement's grid wins");
        assert_eq!(stored.paths, &["M0 0L20 0L20 20Z"]);

        // Capacity: the occupied slot plus `MAX - 1` more fill it, and the next is refused.
        for index in 1..MAX_REGISTERED_ICONS {
            let name: &'static str = Box::leak(format!("icon{index}").into_boxed_str());
            assert!(register_icon(name, &["M0 0L1 0L1 1Z"]), "slot {index} must be accepted");
        }
        assert_eq!(registered_icon_count(), MAX_REGISTERED_ICONS);
        assert!(
            !register_icon("overflow", &["M0 0L1 0L1 1Z"]),
            "a full registry must refuse, not evict"
        );

        assert_eq!(clear_registered_icons(), MAX_REGISTERED_ICONS);
        assert_eq!(registered_icon_count(), 0);
        assert!(!is_registered_icon("mine"), "clearing removes the lookups too");
    }
}
