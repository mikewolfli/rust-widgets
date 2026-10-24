// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Test-only variable faces.
//!
//! # What these are
//!
//! Two minimal fonts whose variation is known exactly, so the variable-font tests can assert
//! geometry rather than "something changed":
//!
//! * [`VARIABLE_WEIGHT_TTF`] has one `wght` axis spanning `100..=900` with its default at `400`,
//!   and an `A` whose 700 master widens the box's right edge by 100 units, raises its top by 100
//!   units and advances 60 units further. The deltas therefore move the outline *and* the advance.
//! * [`VARIABLE_WIDTH_TTF`] is the same shape with a `wdth` axis and **no** `wght` axis, which is
//!   the case a weight request must leave on the face's default instance.
//! * [`VARIABLE_AVAR_TTF`] is [`VARIABLE_WEIGHT_TTF`] with an `avar` segment map for `wght`:
//!   normalized `0.2` is remapped to `0.6`. A user coordinate of `500` therefore reaches the same
//!   instance as `700`, which is what proves the remapping table is consulted before the deltas.
//!
//! # Provenance
//!
//! Both were built for this crate from two interpolation masters per face (a default master and a
//! moved-coordinate master) and contain no third-party outlines: an `A` drawn as a four-point box
//! plus `.notdef`, a `cmap` that maps `A`, and the tables a font parser needs (`head`, `hhea`,
//! `maxp`, `hmtx`, `cmap`, `loca`, `glyf`, `name`, `OS/2`, `post`) alongside `fvar`, `gvar`,
//! `HVAR` and `STAT`. They are the crate's own work and carry the crate's MIT licence.
//!
//! They are compiled only under `cfg(test)`, so no shipped binary carries them; they live in this
//! directory because `tools/check_font_data_is_opt_in.sh` requires every embedded font payload to
//! have its home here, which is the rule that keeps font data out of a caller's build.

/// A variable face with a `wght` axis (`100..=900`, default `400`).
pub(crate) const VARIABLE_WEIGHT_TTF: &[u8] = include_bytes!("variable_test.ttf");

/// A variable face with a `wdth` axis (`75..=125`, default `100`) and no `wght` axis.
pub(crate) const VARIABLE_WIDTH_TTF: &[u8] = include_bytes!("variable_test_wdth.ttf");

/// [`VARIABLE_WEIGHT_TTF`] with an `avar` map that turns normalized `0.2` into `0.6`.
pub(crate) const VARIABLE_AVAR_TTF: &[u8] = include_bytes!("variable_test_avar.ttf");
