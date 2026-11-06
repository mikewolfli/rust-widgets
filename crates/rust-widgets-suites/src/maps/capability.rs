// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The capability rows, property schemas and constructors of this suite group.
//!
//! A capability is the designer's entire view of a control: the name it is created by, the
//! property contract with a type and an accessibility flag per row, and the published events
//! with their payload kinds. All of those are declared here, next to the constructors, so the
//! registration and the documentation cannot describe different controls.

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::apply_control_label;
use rust_widgets::widget::capability::types::{
    CapabilityValue, EventPayloadShape as S, EventSchema, PropertySchema, PropertyValueKind,
    PropertyValueKind as K, RegistrationError, WidgetCapability, WidgetCtor,
};
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::{Widget, WidgetKind};

use crate::maps::map_view::MapView;

pub const MAP_VIEW_PROPERTIES: &[PropertySchema] = &[
    // The marker list as records (BLUE26 B26.09): `set_markers` takes a list the property
    // API could not carry, so the list itself is the property.
    PropertySchema::records("markers", true, true, &crate::maps::map_view::MAP_MARKER_SCHEMA),
    PropertySchema::new("center_x", PropertyValueKind::Float, true, true),
    PropertySchema::new("center_y", PropertyValueKind::Float, true, true),
    PropertySchema::new("zoom", PropertyValueKind::Float, true, true),
    PropertySchema::new("marker_count", PropertyValueKind::UInt, true, false),
    PropertySchema::new("selected_marker_id", PropertyValueKind::String, true, false),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::enumerated("projection", true, true, &["linear", "mercator"]),
];

/// Map overview (COMP-A7). The overview mirrors an attached map, so `map_view_attached`
/// reports that connection rather than being a setting.
pub const MAP_OVERVIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("viewport_ratio", PropertyValueKind::Float, true, true),
    PropertySchema::new("collapsed", PropertyValueKind::Bool, true, true),
    PropertySchema::new("map_view_attached", PropertyValueKind::Bool, true, false),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// Map attribution overlay (COMP-A3). The entries are the text the tile source requires,
/// so only their count is published; `corner` selects the placement.
pub const MAP_ATTRIBUTION_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("entry_count", PropertyValueKind::UInt, true, false),
    PropertySchema::new("corner", PropertyValueKind::String, true, true),
    PropertySchema::new("background_opacity", PropertyValueKind::UInt, true, true),
    PropertySchema::new("font_scale", PropertyValueKind::Float, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// Tile addressing and cache for a map (`COMP-13`).
pub const TILE_LOADER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("url_template", PropertyValueKind::String, true, true),
    PropertySchema::new("min_zoom", PropertyValueKind::UInt, true, true),
    PropertySchema::new("max_zoom", PropertyValueKind::UInt, true, true),
    PropertySchema::new("tile_size", PropertyValueKind::UInt, true, true),
    PropertySchema::new("cache_limit", PropertyValueKind::UInt, true, true),
    PropertySchema::new("cache_size", PropertyValueKind::UInt, true, false),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// Three-button zoom strip for a map (`COMP-13`).
pub const ZOOM_CONTROL_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("zoom_level", PropertyValueKind::Float, true, true),
    PropertySchema::new("min", PropertyValueKind::Float, true, true),
    PropertySchema::new("max", PropertyValueKind::Float, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// Distance legend for a map (`COMP-13`).
pub const SCALE_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("meters_per_pixel", PropertyValueKind::Float, true, true),
    PropertySchema::enumerated("units", true, true, &["metric", "imperial"]),
    PropertySchema::enumerated("style", true, true, &["plain", "boxed"]),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// Bearing dial for a map (`COMP-13`).
pub const COMPASS_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("bearing_deg", PropertyValueKind::Float, true, true),
    PropertySchema::new("clickable", PropertyValueKind::Bool, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// `map_view`'s published events.
const MAP_VIEW_EVENTS: &[EventSchema] = &[
    EventSchema { name: "center_changed", payload: Some(K::Float), shape: Some(S::Tuple2) },
    EventSchema { name: "cluster_clicked", payload: Some(K::Float), shape: Some(S::Tuple2) },
    EventSchema { name: "marker_clicked", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "marker_selected", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "tile_requested", payload: Some(K::String), shape: Some(S::Mixed) },
    EventSchema { name: "zoom_changed", payload: Some(K::Float), shape: Some(S::Scalar) },
];

/// `map_overview`'s published events.
const MAP_OVERVIEW_EVENTS: &[EventSchema] =
    &[EventSchema { name: "on_overview_drag", payload: Some(K::Float), shape: Some(S::Tuple2) }];

/// `map_attribution`'s published events.
const MAP_ATTRIBUTION_EVENTS: &[EventSchema] =
    &[EventSchema { name: "on_entry_activated", payload: Some(K::String), shape: Some(S::Scalar) }];

/// `tile_loader`'s published events.
const TILE_LOADER_EVENTS: &[EventSchema] = &[
    EventSchema { name: "tile_failed", payload: Some(K::Int), shape: Some(S::Tuple3) },
    EventSchema { name: "tile_loaded", payload: Some(K::Int), shape: Some(S::Tuple3) },
];

/// `zoom_control`'s published events.
const ZOOM_CONTROL_EVENTS: &[EventSchema] =
    &[EventSchema { name: "zoom_requested", payload: Some(K::Float), shape: Some(S::Scalar) }];

/// `compass`'s published events.
const COMPASS_EVENTS: &[EventSchema] = &[EventSchema::unit("reset_north_requested")];

pub fn map_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Canvas,
        canonical_name: "map_view",
        aliases: &[],
        properties: MAP_VIEW_PROPERTIES,
        events: MAP_VIEW_EVENTS,
        // set_markers now carries its list through the `markers` records property (B26.03); the
        // bare command reports the value is what is missing.
        commands: &["set_center_x", "set_center_y", "set_zoom", "set_markers"],
    }
}

/// Map overview (COMP-A7).
///
/// It shares `WidgetKind::Canvas` with the scene canvas and the attribution overlay.
pub fn map_overview_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Canvas,
        canonical_name: "map_overview",
        aliases: &[],
        properties: MAP_OVERVIEW_PROPERTIES,
        events: MAP_OVERVIEW_EVENTS,
        commands: &[],
    }
}

/// Map attribution overlay (COMP-A3).
///
/// It shares `WidgetKind::Canvas` with the map's own scene canvas and with the overview below, so
/// each of the three needs its own concrete-type tie-break.
pub fn map_attribution_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Canvas,
        canonical_name: "map_attribution",
        aliases: &[],
        properties: MAP_ATTRIBUTION_PROPERTIES,
        events: MAP_ATTRIBUTION_EVENTS,
        commands: &[],
    }
}

/// `tile_loader` — tile addresses and their cache (`COMP-13`).
pub fn tile_loader_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TileLoader,
        canonical_name: "tile_loader",
        aliases: &["map_tile_loader"],
        properties: TILE_LOADER_PROPERTIES,
        events: TILE_LOADER_EVENTS,
        commands: &["clear_cache"],
    }
}

/// `zoom_control` — the map's three-button zoom strip (`COMP-13`).
pub fn zoom_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ZoomControl,
        canonical_name: "zoom_control",
        aliases: &["map_zoom_control"],
        properties: ZOOM_CONTROL_PROPERTIES,
        events: ZOOM_CONTROL_EVENTS,
        commands: &[],
    }
}

/// `scale_bar` — the map's distance legend (`COMP-13`).
pub fn scale_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ScaleBar,
        canonical_name: "scale_bar",
        aliases: &["map_scale_bar"],
        properties: SCALE_BAR_PROPERTIES,
        events: &[],
        commands: &[],
    }
}

/// `compass` — the map's north-up dial (`COMP-13`).
pub fn compass_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Compass,
        canonical_name: "compass",
        aliases: &["map_compass"],
        properties: COMPASS_PROPERTIES,
        events: COMPASS_EVENTS,
        commands: &[],
    }
}

/// Creates a map view with no tiles or markers. `text` is applied as the
/// control's label.
pub fn create_map_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(MapView::new(geometry)), text)
}

/// Creates an uncollapsed map overview with no map attached (COMP-A7). `text` is applied as the
/// control's label.
pub fn create_map_overview(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(crate::maps::map_overview::MapOverviewPanel::new(geometry)), text)
}

/// Creates an empty map attribution overlay (COMP-A3). `text` is applied as the control's label.
pub fn create_map_attribution(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(
        Box::new(crate::maps::map_attribution::MapAttributionOverlay::new(geometry)),
        text,
    )
}

/// Creates a tile loader with the default template (`COMP-13`).
pub fn create_tile_loader(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(crate::maps::map_view::TileLoader::new(geometry))
}

/// Creates a map zoom strip at 1:1 (`COMP-13`).
pub fn create_zoom_control(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(crate::maps::map_view::ZoomControl::new(geometry))
}

/// Creates a metric scale bar (`COMP-13`).
pub fn create_scale_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(crate::maps::map_view::ScaleBar::new(geometry))
}

/// Creates a clickable compass pointing north (`COMP-13`).
pub fn create_compass(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(crate::maps::map_view::Compass::new(geometry))
}

/// A registered control: its capability row, its constructor, and its defaults provider.
type MapRow = (WidgetCapability, WidgetCtor, fn(&str) -> Option<CapabilityValue>);

/// Registers the maps suite in `factory`.
///
/// Returns the first name that could not be claimed. A factory that already answers one of these
/// names (a host that installed twice, or a build that ships a control of the same name) is
/// reported rather than silently shadowed.
pub fn install(factory: &mut WidgetFactory) -> Result<(), RegistrationError> {
    let rows: [MapRow; 7] = [
        (map_view_capability(), create_map_view, |p| default_property_value("map_view", p)),
        (map_overview_capability(), create_map_overview, |p| {
            default_property_value("map_overview", p)
        }),
        (map_attribution_capability(), create_map_attribution, |p| {
            default_property_value("map_attribution", p)
        }),
        (tile_loader_capability(), create_tile_loader, |p| {
            default_property_value("tile_loader", p)
        }),
        (zoom_control_capability(), create_zoom_control, |p| {
            default_property_value("zoom_control", p)
        }),
        (scale_bar_capability(), create_scale_bar, |p| default_property_value("scale_bar", p)),
        (compass_capability(), create_compass, |p| default_property_value("compass", p)),
    ];
    for (capability, ctor, defaults) in rows {
        let name = capability.canonical_name;
        factory.try_register(capability, ctor)?;
        let _ = factory.register_property_defaults(name, defaults);
    }
    Ok(())
}

/// A property's declared default, for the capability manifest.
///
/// Mirrors what the controls' constructors set, so a dropped binding resets to the value a fresh
/// control would have had.
pub fn default_property_value(kind: &str, name: &str) -> Option<CapabilityValue> {
    match kind {
        "map_view" => match name {
            // A fresh map has no markers; the empty list is what a dropped binding resets to.
            "markers" => {
                Some(CapabilityValue::Records(rust_widgets::widget::capability::RecordList::new(
                    &crate::maps::map_view::MAP_MARKER_SCHEMA,
                )))
            }
            "center_x" | "center_y" => Some(CapabilityValue::Float(0.0)),
            "zoom" => Some(CapabilityValue::Float(1.0)),
            "marker_count" | "layer_count" => Some(CapabilityValue::UInt(0)),
            "selected_marker_id" => Some(CapabilityValue::Null),
            "projection" => Some(CapabilityValue::String("linear".to_string())),
            "anti_alias" => Some(CapabilityValue::Bool(true)),
            "show_grid" | "snap_to_grid" => Some(CapabilityValue::Bool(false)),
            "grid_step" => Some(CapabilityValue::UInt(20)),
            "zoom_range" => Some(CapabilityValue::Tuple(rust_widgets::compat::Vec::from([
                CapabilityValue::Float(0.125),
                CapabilityValue::Float(16.0),
            ]))),
            "zoom_step" => Some(CapabilityValue::Float(1.1)),
            _ => None,
        },
        "map_overview" => match name {
            // The constructor's initial values: a quarter of the extent, expanded, and no map
            // attached yet. RW-105: the three rows had no branch, so the whole manifest failed
            // to export and the designer could not see the control at all.
            "viewport_ratio" => Some(CapabilityValue::Float(0.25)),
            "collapsed" | "map_view_attached" => Some(CapabilityValue::Bool(false)),
            _ => None,
        },
        "map_attribution" => match name {
            // A fresh overlay is empty, anchored top-left, and painted at `160/255` opacity —
            // the values `MapAttributionOverlay::new` sets (RW-105).
            "entry_count" => Some(CapabilityValue::UInt(0)),
            "corner" => Some(CapabilityValue::String("top_left".to_string())),
            "background_opacity" => Some(CapabilityValue::UInt(160)),
            "font_scale" => Some(CapabilityValue::Float(1.0)),
            _ => None,
        },
        "tile_loader" => match name {
            "url_template" => Some(CapabilityValue::String("tiles/{z}/{x}/{y}.png".to_string())),
            "min_zoom" => Some(CapabilityValue::UInt(0)),
            "max_zoom" => Some(CapabilityValue::UInt(19)),
            "tile_size" => Some(CapabilityValue::UInt(256)),
            "cache_limit" => Some(CapabilityValue::UInt(256)),
            "cache_size" => Some(CapabilityValue::UInt(0)),
            _ => None,
        },
        "zoom_control" => match name {
            "zoom_level" => Some(CapabilityValue::Float(1.0)),
            "min" => Some(CapabilityValue::Float(0.25)),
            "max" => Some(CapabilityValue::Float(8.0)),
            _ => None,
        },
        "scale_bar" => match name {
            "meters_per_pixel" => Some(CapabilityValue::Float(1.0)),
            "units" => Some(CapabilityValue::String("metric".to_string())),
            "style" => Some(CapabilityValue::String("plain".to_string())),
            _ => None,
        },
        "compass" => match name {
            "bearing_deg" => Some(CapabilityValue::Float(0.0)),
            "clickable" => Some(CapabilityValue::Bool(true)),
            _ => None,
        },
        _ => None,
    }
}

/// Every property name the maps suite publishes, for a quick completeness check.
pub fn published_property_names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("map_view", MAP_VIEW_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("map_overview", MAP_OVERVIEW_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("map_attribution", MAP_ATTRIBUTION_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("tile_loader", TILE_LOADER_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("zoom_control", ZOOM_CONTROL_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("scale_bar", SCALE_BAR_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("compass", COMPASS_PROPERTIES.iter().map(|schema| schema.name).collect()),
    ]
}

/// Every event name the maps suite publishes, for a quick completeness check.
pub fn published_event_names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("map_view", MAP_VIEW_EVENTS.iter().map(|schema| schema.name).collect()),
        ("map_overview", MAP_OVERVIEW_EVENTS.iter().map(|schema| schema.name).collect()),
        ("map_attribution", MAP_ATTRIBUTION_EVENTS.iter().map(|schema| schema.name).collect()),
        ("tile_loader", TILE_LOADER_EVENTS.iter().map(|schema| schema.name).collect()),
        // `scale_bar` is a distance legend with no interaction of its own, so it publishes none.
        ("scale_bar", Vec::new()),
        ("zoom_control", ZOOM_CONTROL_EVENTS.iter().map(|schema| schema.name).collect()),
        ("compass", COMPASS_EVENTS.iter().map(|schema| schema.name).collect()),
    ]
}
