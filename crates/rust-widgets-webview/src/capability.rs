// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The capability rows, property schemas and constructors of the web controls.
//!
//! These describe the same public names the parent crate used to publish
//! (`web_engine_view` with its `web_view`/`webview` aliases, `web_find_bar`,
//! `web_history_panel`). They moved here with the implementations: a capability row
//! describes a constructor, and a row whose control lives in another package cannot be
//! registered by that package's default factory.
//!
//! Register them with [`install`].

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::types::{
    CapabilityValue, EventPayloadShape as S, EventSchema, PropertySchema,
    PropertyValueKind, PropertyValueKind as K, RegistrationError, WidgetCapability,
    WidgetCtor,
};
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::{Widget, WidgetKind};

use crate::find_bar::WebFindBar;
use crate::web_engine::WebEngineView;
use crate::web_history_panel::WebHistoryPanel;

        pub(super) const WEB_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("url", PropertyValueKind::String, true, true),
    PropertySchema::new("loading", PropertyValueKind::Bool, true, false),
    PropertySchema::new("title", PropertyValueKind::String, true, true),
    PropertySchema::new("can_go_back", PropertyValueKind::Bool, false, false),
    PropertySchema::new("can_go_forward", PropertyValueKind::Bool, false, false),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::new("user_agent", PropertyValueKind::String, true, true),
    PropertySchema::new("background_transparent", PropertyValueKind::Bool, true, true),
    PropertySchema::new("cookie_persistence_enabled", PropertyValueKind::Bool, true, true),
];

        pub(super) const WEB_FIND_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("query", PropertyValueKind::String, true, true),
    PropertySchema::new("case_sensitive", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("match_index", PropertyValueKind::UInt, true, true),
    PropertySchema::new("match_count", PropertyValueKind::UInt, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

        pub(super) const WEB_HISTORY_PANEL_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("max_entries", PropertyValueKind::UInt, true, true),
    PropertySchema::new("show_timestamps", PropertyValueKind::Bool, true, true),
    PropertySchema::new("group_by_day", PropertyValueKind::Bool, true, true),
    PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
    PropertySchema::new("filter_prefix", PropertyValueKind::String, true, true),
    PropertySchema::new("visible_count", PropertyValueKind::UInt, true, false),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// The web engine view's published events.
const WEB_ENGINE_VIEW_EVENTS: &[EventSchema] = &[
    EventSchema { name: "certificate_error", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "channel_message", payload: Some(K::String), shape: Some(S::Tuple2) },
    EventSchema { name: "console_message", payload: Some(K::String), shape: Some(S::Mixed) },
    EventSchema {
        name: "download_progress_changed",
        payload: Some(K::String),
        shape: Some(S::Mixed),
    },
    EventSchema { name: "download_requested", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "error_occurred", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "js_evaluated", payload: Some(K::String), shape: Some(S::Mixed) },
    EventSchema { name: "link_activated", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "link_hovered", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "load_progress_changed", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "loading_finished", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "loading_started", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema {
        name: "navigation_state_changed",
        payload: Some(K::Bool),
        shape: Some(S::Tuple2),
    },
    EventSchema { name: "on_window_requested", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "page_created", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "page_destroyed", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema {
        name: "render_process_crashed",
        payload: Some(K::String),
        shape: Some(S::Scalar),
    },
    EventSchema { name: "title_changed", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "url_changed", payload: Some(K::String), shape: Some(S::Scalar) },
];

/// The find strip's published events.
const WEB_FIND_BAR_EVENTS: &[EventSchema] = &[
    EventSchema { name: "closed", payload: None, shape: None },
    EventSchema { name: "next_requested", payload: None, shape: None },
    EventSchema { name: "previous_requested", payload: None, shape: None },
    EventSchema { name: "search_requested", payload: Some(K::String), shape: Some(S::Mixed) },
];

/// The history panel's published events.
const WEB_HISTORY_PANEL_EVENTS: &[EventSchema] = &[
    EventSchema { name: "on_entry_activated", payload: Some(K::String), shape: Some(S::Scalar) },
];

/// Sets a control's label through the property API, the way the parent crate's factory
/// constructors do.
fn label(text: &str, mut widget: Box<dyn Widget>) -> Box<dyn Widget> {
    if text.is_empty() {
        return widget;
    }
    if let Some(properties) = widget.properties_dyn_mut() {
        let _ = properties.set("text", CapabilityValue::String(text.to_string()));
    }
    widget
}

/// Creates a web engine view with no page loaded.
pub fn create_web_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(WebEngineView::new(geometry))
}

/// Creates a closed find strip.
pub fn create_web_find_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(WebFindBar::new(geometry))
}

/// Creates a history panel with no entries.
pub fn create_web_history_panel(geometry: Rect, text: &str) -> Box<dyn Widget> {
    label(text, Box::new(WebHistoryPanel::new(geometry)))
}

/// The web view's capability.
pub fn web_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebEngineView,
        canonical_name: "web_engine_view",
        aliases: &["webview", "web_view"],
        properties: WEB_VIEW_PROPERTIES,
        events: WEB_ENGINE_VIEW_EVENTS,
        commands: &["set_url", "reload", "go_back", "go_forward", "stop"],
    }
}

/// The find strip's capability.
pub fn web_find_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebFindBar,
        canonical_name: "web_find_bar",
        aliases: &[],
        properties: WEB_FIND_BAR_PROPERTIES,
        events: WEB_FIND_BAR_EVENTS,
        commands: &["next", "previous", "close"],
    }
}

/// The history panel's capability.
pub fn web_history_panel_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebEngineView,
        canonical_name: "web_history_panel",
        aliases: &[],
        properties: WEB_HISTORY_PANEL_PROPERTIES,
        events: WEB_HISTORY_PANEL_EVENTS,
        commands: &[],
    }
}

/// Registers the web controls in `factory`.
///
/// Returns the first name that could not be claimed. A factory that already answers one of
/// these names (a host that installed twice, or a build that still ships the controls itself)
/// is reported rather than silently shadowed.
pub fn install(factory: &mut WidgetFactory) -> Result<(), RegistrationError> {
    let rows: [(WidgetCapability, WidgetCtor); 3] = [
        (web_view_capability(), create_web_view),
        (web_find_bar_capability(), create_web_find_bar),
        (web_history_panel_capability(), create_web_history_panel),
    ];
    for (capability, ctor) in rows {
        factory.try_register(capability, ctor)?;
    }
    Ok(())
}
