// The inner module keeps the control's own name: the folder is the family and the file is
// the control, so the historical `…::map_view::map_view` path stays valid. `module_inception` would
// prefer a different inner name; renaming here would move a published module path for a lint.
#![allow(clippy::module_inception)]
// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Map control family.
//!
//! [`map_view`] is the main control: a tile map with markers, clustering and a projection.
//! [`map_view_companions`] holds the chrome that surrounds it (the tile loader, the zoom
//! control, the scale bar and the compass), and [`map_marker_detail`] is the anchored
//! information window the map opens for a marker. Keeping the three modules in one folder
//! makes the family identifiable at a glance.
//!
//! `map_attribution` and `map_overview` stay standalone controls under `special_widgets`:
//! both mount on their own and the map does not own either of them.

pub mod map_marker_detail;
pub mod map_view;
pub mod map_view_companions;

// Re-export the item surface at the folder root so the paths that predate the folder
// (`special_widgets::map_view::MapView`, `...::TileLoader`, ...) keep resolving unchanged.
pub use map_marker_detail::{DetailLine, MapMarkerDetail};
pub use map_view::{
    Compass, MapMarker, MapProjection, MapView, MarkerStyle, ScaleBar, ScaleBarStyle, ScaleUnits,
    TileKey, TileLoadError, TileLoader, TileRequest, TileViewport, ZoomButton, ZoomControl,
    MAP_MARKER_SCHEMA, MAX_TILE_ZOOM,
};
