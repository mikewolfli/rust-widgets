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

use crate::widget::display_widgets::icon::{IconPaintError, IconRef};
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
/// // The path table must be `&'static [&'static str]` (see "Why the name is `&'static str`"),
/// // so it is a `static` rather than a local binding.
/// static TRIANGLE: [&str; 1] = ["M480-200 240-440l56-56 184 184 184-184 56 56-240 240Z"];
/// assert!(register_icon("disclosure", &TRIANGLE));
/// ```
pub fn register_icon(name: &'static str, paths: &'static [&'static str]) -> bool {
    try_register_icon(name, paths).is_ok()
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
    try_register_icon_on_grid(name, paths, grid).is_ok()
}

/// Why a host icon registration was refused.
///
/// # Why this exists alongside the `bool` return
///
/// `bool` cannot say *which* of half a dozen reasons applied, and a host that registered a
/// broken icon needs the reason: "the registry is full" and "your path data does not parse" call
/// for completely different responses, and the old `false` conflated them with "your name was
/// empty". The `bool` wrappers are unchanged and delegate here, so both spellings answer from one
/// implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconRegistrationError {
    /// The token was empty, so nothing could name the registration.
    EmptyName,
    /// No path data was supplied, so there would be nothing to draw.
    NoPaths,
    /// The design grid is zero, so no device scale can be derived from it.
    ZeroGrid,
    /// A path entry was blank. An empty `d` draws nothing, which is indistinguishable on screen
    /// from a missing icon, so it is refused rather than stored.
    BlankPath,
    /// The registry is full and this name is not already registered.
    ///
    /// A **re-registration** of an existing name is never refused by capacity: it replaces in
    /// place, which is why the name is reported here.
    RegistryFull {
        /// How many icons the registry holds at once.
        capacity: usize,
    },
    /// The geometry could not be drawn. The reason comes from the crate's own source validator, so
    /// a registration is refused exactly when a draw would fail.
    Undrawable(IconPaintError),
}

/// [`register_icon`], reporting **why** a registration was refused.
///
/// # Declared behaviour change
///
/// The geometry is now validated before it is stored, so a registration whose path data cannot be
/// drawn is refused instead of being stored and then silently drawing nothing every frame. This
/// tightens what [`register_icon`] accepts — a host that registered unparseable data used to get
/// `true` and a blank icon, and now gets `false` and a reason. The doc comment on
/// [`register_icon`] always claimed a `false` means "not drawable"; this is what makes that true.
/// A name that is already registered is still replaced, so capacity is not a limit for updates.
pub fn try_register_icon(
    name: &'static str,
    paths: &'static [&'static str],
) -> Result<(), IconRegistrationError> {
    try_register_icon_on_grid(name, paths, 960)
}

/// [`register_icon_on_grid`], reporting **why** a registration was refused.
///
/// See [`try_register_icon`] for the declared behaviour change.
pub fn try_register_icon_on_grid(
    name: &'static str,
    paths: &'static [&'static str],
    grid: u16,
) -> Result<(), IconRegistrationError> {
    use IconRegistrationError as Refused;

    if name.is_empty() {
        return Err(Refused::EmptyName);
    }
    if paths.is_empty() {
        return Err(Refused::NoPaths);
    }
    if grid == 0 {
        return Err(Refused::ZeroGrid);
    }
    if paths.iter().any(|d| d.trim().is_empty()) {
        return Err(Refused::BlankPath);
    }
    // Validate before storing, through the same validator the paint path uses, so "accepted" and
    // "will draw" cannot diverge.
    let data = crate::widget::IconData { name, grid, paths };
    crate::widget::display_widgets::icon::validate_icon(IconRef::StaticGeometry(&data))
        .map_err(Refused::Undrawable)?;
    // Capacity only refuses a **new** name; a re-registration replaces in place, so it is checked
    // after the replacement case is excluded rather than before.
    if !is_registered_icon(name) && registered_icon_count() >= MAX_REGISTERED_ICONS {
        return Err(Refused::RegistryFull { capacity: MAX_REGISTERED_ICONS });
    }
    if register_icon_data(name, paths, grid) {
        Ok(())
    } else {
        // Unreachable while the capacity check above and the storage check agree; reported as a
        // capacity refusal rather than `unreachable!()` because this is a public entry point.
        Err(Refused::RegistryFull { capacity: MAX_REGISTERED_ICONS })
    }
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

        // ── The `try_` pair says **which** reason applied ──
        //
        // The legacy `bool` conflated "the registry is full" with "your name was empty" and "your
        // path data does not parse", and a host cannot react to those the same way. The names below
        // are new, so the capacity assertions above stay meaningful.
        assert!(
            try_register_icon("mine", &["M0 0L1 0L1 1Z"]).is_ok(),
            "re-registering an *existing* name replaces in place, so a full registry still accepts it"
        );
        assert_eq!(
            try_register_icon("overflow-new", &["M0 0L1 0L1 1Z"]),
            Err(IconRegistrationError::RegistryFull { capacity: MAX_REGISTERED_ICONS })
        );
        assert_eq!(
            try_register_icon("", &["M0 0L1 0L1 1Z"]),
            Err(IconRegistrationError::EmptyName)
        );
        assert_eq!(try_register_icon("x", &[]), Err(IconRegistrationError::NoPaths));
        assert_eq!(
            try_register_icon_on_grid("x", &["M0 0L1 0L1 1Z"], 0),
            Err(IconRegistrationError::ZeroGrid)
        );
        assert_eq!(try_register_icon("x", &["   "]), Err(IconRegistrationError::BlankPath));

        // Geometry that cannot be drawn is refused **with the validator's own reason**, instead of
        // being stored and then silently drawing nothing every frame.
        assert!(matches!(
            try_register_icon("broken", &["M0 0L oops"]),
            Err(IconRegistrationError::Undrawable(
                crate::widget::display_widgets::icon::IconPaintError::InvalidGeometry(_)
            ))
        ));
        // `M0 0` is a legitimate `d` that declares no contour, which is a different refusal from
        // `BlankPath` ("nothing was typed") -- the distinction `bool` could not make.
        assert!(matches!(
            try_register_icon("no-contour", &["M0 0"]),
            Err(IconRegistrationError::Undrawable(
                crate::widget::display_widgets::icon::IconPaintError::EmptyGeometry
            ))
        ));
        assert_eq!(
            registered_icon_count(),
            MAX_REGISTERED_ICONS,
            "a refused registration must not occupy a slot"
        );

        assert_eq!(clear_registered_icons(), MAX_REGISTERED_ICONS);
        assert_eq!(registered_icon_count(), 0);
        assert!(!is_registered_icon("mine"), "clearing removes the lookups too");
    }
}
