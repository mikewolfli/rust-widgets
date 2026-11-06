// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Material Symbols outlined icon geometry for `rust_widgets`.
//!
//! # What this crate owns
//!
//! The icon **resources** of the widget library live here so the main crate's source carries no
//! generated third-party data: the `IconName` name table (the hand-curated tokens plus the whole
//! Material Symbols catalogue), the SVG path (`d`) table, the flattened fallback polygons, and the
//! record of which names upstream draws with one picture. The main crate's `Icon` control draws
//! them; it does not define them.
//!
//! # Features
//!
//! * `data` (off by default) — compile [`icon_data`], the SVG path table. `rust_widgets`'s
//!   `icons` feature turns this on for the capable device profiles.
//! * Without `data`, the crate still exposes the name table and [`icon_fallback`]. The fallback is
//!   **layered**: the curated tokens a control in this crate names directly carry polygon
//!   geometry, and a catalogue token has an empty entry that reports "no geometry here" — the
//!   alternative was carrying ~12 MB of constants into every build, `mini`/`embedded` included.
//!   See `fallback.rs`'s header for the numbers.
//!
//! # Licence
//!
//! The Rust code is MIT. The geometry is a derivative of Material Symbols (Google LLC,
//! Apache-2.0); see `NOTICE` and `LICENSE-APACHE`.

#![no_std]

mod alias;
#[cfg(feature = "data")]
mod data;
mod fallback;
mod names;
mod upstream_aliases;

pub use alias::{designer_alias, is_designer_alias, DESIGNER_ALIASES};
pub use fallback::{IconFallback, FALLBACK_GEOMETRY_ENTRIES};
pub use names::IconName;
pub use upstream_aliases::UPSTREAM_ALIASES;

/// One icon's outline, as SVG path data on a square design grid.
///
/// # Coordinates
///
/// Material Symbols draws on a 960-unit grid with **negative y upward** (its own `viewBox` is
/// `0 -960 960 960`). This keeps upstream's numbers verbatim; the drawing code scales and flips
/// the axis, so the data stays a copy rather than a re-encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconData {
    /// The canonical token, matching [`IconName::as_str`].
    pub name: &'static str,
    /// The design grid: `x` spans `0..grid`, `y` spans `-grid..0`.
    pub grid: u16,
    /// The SVG path data (`d`), one entry per `<path>` in the upstream file.
    pub paths: &'static [&'static str],
}

/// Every icon's SVG path data, indexed by [`IconName`]'s declaration order.
///
/// Available only with the `data` feature; a build without it uses [`icon_fallback`].
#[cfg(feature = "data")]
pub const fn icon_data() -> &'static [IconData] {
    &data::ICON_DATA
}

/// Every icon's flattened fallback polygons, indexed by [`IconName`]'s declaration order.
///
/// This is always compiled: it is the geometry a build without the `data` feature draws. Its
/// entries are **dense** — one per token — but only the first [`FALLBACK_GEOMETRY_ENTRIES`]
/// carry polygons; a catalogue token's entry is empty and reports "no fallback geometry here"
/// (see `fallback.rs`'s header for why the table is layered rather than complete).
pub const fn icon_fallback() -> &'static [IconFallback] {
    &fallback::ICON_FALLBACK
}
