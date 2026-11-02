// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `maps` group: the map view, its companion chrome and the two standalone map overlays.
//!
//! # Why these are one group
//!
//! The controls share one geometry: a map view, the tile loader that feeds it, the zoom strip, the
//! scale bar and the compass that surround it, and the overview/attribution overlays that read the
//! same viewport. A host installs them together or not at all, so they ship as one feature.
//!
//! # Adding a group later
//!
//! A group is a module under `src/` with a `capability` submodule holding its property schemas,
//! event schemas and constructors, and an `install(factory)` function. A `mod.rs` only declares
//! and re-exports (principle #8).

mod capability;
/// The attribution overlay: the data-source credit line map providers require.
pub mod map_attribution;
/// The overview panel: a miniature of the full map with a viewport rectangle.
pub mod map_overview;
/// The map view itself, with its markers, projection and companion chrome.
pub mod map_view;

pub use capability::{
    compass_capability, create_compass, create_map_attribution, create_map_overview,
    create_map_view, create_scale_bar, create_tile_loader, create_zoom_control,
    default_property_value, install, map_attribution_capability, map_overview_capability,
    map_view_capability, published_event_names, published_property_names, scale_bar_capability,
    tile_loader_capability, zoom_control_capability, COMPASS_PROPERTIES,
    MAP_ATTRIBUTION_PROPERTIES, MAP_OVERVIEW_PROPERTIES, MAP_VIEW_PROPERTIES, SCALE_BAR_PROPERTIES,
    TILE_LOADER_PROPERTIES, ZOOM_CONTROL_PROPERTIES,
};
pub use map_attribution::{AttributionEntry, Corner, MapAttributionOverlay};
pub use map_overview::{CenterMarkerStyle, MapOverviewPanel};
pub use map_view::{map_marker_detail, map_view_companions};
pub use map_view::{
    Compass, MapMarker, MapProjection, MapView, MarkerStyle, ScaleBar, ScaleBarStyle, ScaleUnits,
    TileKey, TileLoadError, TileLoader, TileRequest, TileViewport, ZoomButton, ZoomControl,
    MAP_MARKER_SCHEMA, MAX_TILE_ZOOM,
};
