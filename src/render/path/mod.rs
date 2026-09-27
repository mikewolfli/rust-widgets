// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Path geometry shared by the vector backends and the icon data.
//!
//! Two members, two jobs: [`parser`] reads SVG path data into curve segments, and [`flatten`] turns
//! those segments into device-space polylines for the fill. Flattening is *not* re-implemented in
//! [`flatten`] — the deviation-adaptive subdivider lives in [`crate::render::text::raster`] and is
//! reused, so there is a single curve-to-polyline rule in the crate (principle #101).

pub mod flatten;
pub mod parser;

pub use flatten::{
    flatten_paths, FlattenError, IconPlacement, MAX_OUTLINE_CONTOURS, MAX_OUTLINE_POINTS,
};
pub use parser::{parse, PathError, Point, Segment, Subpath, MAX_PATH_POINTS};
