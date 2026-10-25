// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! One theme read per draw for the container family (issue C-P01).
//!
//! # Why this module exists
//!
//! Every container `draw` used to resolve its chrome in two independent steps: the
//! resolved style for the control ([`crate::style::resolved_theme_style_shared`]) and a
//! second visit to the global theme manager for the window fill
//! (`theme_manager().current_theme()`). The global manager's mutex is not re-entrant, so
//! the steps could not be merged at the call site without holding a guard across a
//! nested lookup — and containers are the most frequently drawn controls in a tree, so
//! the extra global lock was paid per container per frame.
//!
//! [`ThemeSnapshot::capture`] performs the read once: it copies the palette roles a
//! container needs out of a single manager guard, and it takes the control's resolved
//! style from the theme's own cache (a cache hit does not touch the manager at all).
//! Draw helpers then receive `&ThemeSnapshot` and never lock again.
//!
//! # Semantics are unchanged
//!
//! The snapshot keeps "no active theme" distinguishable ([`ThemeSnapshot::palette`] is
//! `None`) instead of fabricating a palette, so each control's existing
//! "explicit style → theme style → theme palette → literal" ladder keeps its exact
//! precedence and its exact fallback literals.

use crate::compat::Rc;
use crate::core::Color;
use crate::style::WidgetStyle;

/// The palette roles a container's chrome derives from.
///
/// A plain `Copy` snapshot of the active theme's colours: the manager guard is released
/// as soon as this value is built, which is what makes it safe to hold across the rest
/// of a draw (`RenderContext` calls no theme code today, but a future one must not
/// deadlock against a guard this module kept alive).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Palette {
    /// Window / page background.
    pub(crate) background: Color,
    /// Default ink colour.
    pub(crate) foreground: Color,
    /// Accent (selection, active mark).
    pub(crate) accent: Color,
    /// Primary brand/action colour (a stronger accent than [`Palette::accent`]).
    pub(crate) primary: Color,
    /// Secondary neutral colour (a second chrome weight).
    pub(crate) secondary: Color,
    /// Raised container colour, one step above the window fill.
    pub(crate) surface_container_high: Color,
}

/// A container's resolved theme state for one frame.
pub(crate) struct ThemeSnapshot {
    /// The control's resolved style, taken from the theme cache (shared, not cloned).
    pub(crate) style: Option<Rc<WidgetStyle>>,
    /// The active palette, or `None` when no theme is active / the build has no theme.
    pub(crate) palette: Option<Palette>,
}

impl ThemeSnapshot {
    /// Reads the theme once for a container named `widget_name`.
    pub(crate) fn capture(widget_name: &str) -> Self {
        let style = crate::style::resolved_theme_style_shared(widget_name);
        // One manager visit for every palette role used by the container family. The
        // guard is dropped at the end of this block; see [`Palette`].
        let palette = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|theme| Palette {
                background: theme.colors.background,
                foreground: theme.colors.foreground,
                accent: theme.colors.accent,
                primary: theme.colors.primary,
                secondary: theme.colors.secondary,
                surface_container_high: theme.colors.surface_container_high,
            })
        };
        Self { style, palette }
    }

    /// The window fill, falling back to white when no theme is active.
    ///
    /// This is the value the family used to read through a second manager lock.
    pub(crate) fn window_fill(&self) -> Color {
        self.palette.map_or(Color::WHITE, |palette| palette.background)
    }

    /// The accent colour, if a theme is active.
    pub(crate) fn accent(&self) -> Option<Color> {
        self.palette.map(|palette| palette.accent)
    }

    /// The raised container colour, if a theme is active.
    pub(crate) fn surface_container_high(&self) -> Option<Color> {
        self.palette.map(|palette| palette.surface_container_high)
    }
}
