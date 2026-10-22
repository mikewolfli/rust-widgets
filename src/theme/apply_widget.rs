// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The public styling entry point for tools that render outside a creation funnel.
//!
//! Kept out of `mod.rs` (principle #8). Both `#[cfg]` arms live here so the pair is read
//! together — a missing arm is how a profile ends up with an unresolved symbol.

/// Applies the active theme's style for `widget` to `widget`, as the creation funnels do.
///
/// # Why this is public
///
/// `apply_active_theme` is crate-private because the funnels are the only *production*
/// callers, but a tool that renders a control **outside** a funnel needs the same styling
/// or it renders something no user will ever see. The BLUE20 layer 3 SVG exporter was
/// exactly that case: it rendered every control with the theme never applied, so
/// `<name>.svg` and `<name>.light.svg` came out byte-identical apart from their comment —
/// the snapshots existed, looked right in a file listing, and proved nothing.
///
/// This is a single entry point rather than a second implementation: it resolves the
/// style through the same `manager.resolve_style` the funnels use and merges it the same
/// way, so a caller cannot accidentally get a different palette from a control created the
/// normal way. It is `#[cfg]`-gated the same way `apply_active_theme` is, so a profile
/// without a theme module reports `false` (nothing was applied) instead of pretending.
///
/// Returns `true` when a style was applied, `false` when there is no active theme or the
/// build has no theme module — the honest answer in both cases (principle #37: a missing
/// capability is reported, never faked).
///
/// # Gating
///
/// `not(alloc_frugal)` alone is **not** enough. `alloc_frugal` is about the allocator, and
/// it is false for `mini`/`embedded` — but those profiles have no colour model at all, so
/// `crate::theme` does not exist and naming it fails to compile. The condition that
/// actually matches the module is `full_widgets`, which is what `apply_active_theme` itself
/// is gated on and what `build.rs` defines for "a device profile *and* a full widget set".
/// Copying the neighbouring `#[cfg]` without checking it against the module is exactly the
/// drift principle #47 exists to prevent.
#[cfg(full_widgets)]
pub fn apply_theme_to_widget(widget: &mut dyn crate::widget::Widget) -> bool {
    // `apply_active_theme` already handles "no active theme" by returning without styling,
    // so the emptiness check is only to make the return value truthful.
    if super::global_theme_manager().current_theme().is_none() {
        return false;
    }
    super::apply_active_theme(widget);
    true
}

/// The no-theme arm, for the profiles that compile this module out.
///
/// A separate definition rather than a `cfg` inside one body: the module is not compiled in
/// these profiles, so a caller in the widget layer can still link against the name and get
/// the truthful "nothing was applied" answer instead of an unresolved symbol.
#[cfg(not(full_widgets))]
pub fn apply_theme_to_widget(widget: &mut dyn crate::widget::Widget) -> bool {
    let _ = widget;
    false
}
