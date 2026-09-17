// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Declarative JSON window engine — **PC/Desktop path**.
//!
//! This module provides runtime construction of widget trees from JSON
//! layout declarations. It is the **PC/desktop path** in the dual-path
//! declarative UI strategy:
//!
//! - **PC path (this module)**: JSON runtime loading via `serde_json`,
//!   supports hot-reload, dynamic UI, and design tool integration.
//! - **Embedded path (future)**: Procedural macros at compile time,
//!   zero runtime overhead for MCU/RTOS targets.
//!
//! # Architecture
//!
//! ```text
//!        ┌──────────────────────┐
//!        │  JSON source / string │
//!        └──────────┬───────────┘
//!                   ▼
//!        ┌──────────────────────┐
//!        │  1. Parsing          │  serde_json::Value → DeclarativeNode
//!        └──────────┬───────────┘
//!                   ▼
//!        ┌──────────────────────┐
//!        │  2. Layout layer     │  Parse "layout" objects → LayoutKind
//!        └──────────┬───────────┘        → create Layout trait object
//!                   ▼
//!        ┌──────────────────────┐
//!        │  3. Instantiation    │  DeclarativeNode → Box<dyn Widget>
//!        └──────────┬───────────┘
//!                   ▼
//!        ┌──────────────────────┐
//!        │  4. Binding layer    │  "on_click" → EventHandlerMap
//!        └──────────────────────┘
//! ```
//!
//! # JSON Layout Format
//!
//! ```json
//! {
//!   "window": {
//!     "id": "main",
//!     "title": "Hello",
//!     "width": 400,
//!     "height": 300,
//!     "layout": {
//!       "type": "vbox",
//!       "children": [
//!         { "label": { "id": "greeting", "text": "Hello, World!" } },
//!         { "button": { "id": "btn_ok", "text": "OK" } }
//!       ]
//!     }
//!   }
//! }
//! ```
//!
//! # Reachability
//!
//! **State:** Reserved: a complete declarative loader (162 registered kind names,
//! 10 layouts, property access routed through each control's contract, and CSS
//! integration) whose consumers are this repository's own tests and benchmark.
//! Retained deliberately rather than deleted, because removing it is an
//! irreversible narrowing of scope and this module is the only consumer of three
//! pieces of infrastructure at once: the property contract (`properties.rs`), the
//! layout kinds (`layout.rs`) and the name-to-handle binding (`element.rs`). Its
//! maintenance surface is already near zero: it no longer hand-writes per-control
//! setters or keeps its own kind table — unknown names go to the widget factory.
//!
//! Not duplicated by CSS. `src/style` defines *appearance* (colour, borders,
//! fonts); this module defines *structure* (which controls exist, how they nest).
//! `grep -c "children\|layout" src/style/css.rs` is `0`. The dependency runs one
//! way — this module calls `CssParser` through `Widget::apply_css` — so CSS
//! survives its removal rather than being replaced by it.
//!
//! Removal condition: no JSON-layout consumer appears by the time the declarative
//! path is re-evaluated, and `src/layout/inspector.rs` (the other caller of the
//! structures this module builds) is retired too.

mod element;
mod events;
mod layout;
mod loader;
mod properties;

pub use element::BoundJsonLayout;
pub use events::{
    clear_global_handlers, invoke_global_handler, register_global_handler, EventHandlerContext,
    EventHandlerMap,
};
pub use layout::{
    add_spacer_to_layout, add_widget_to_layout, apply_layout, create_layout_from_kind,
    parse_layout_kind, store_layout, ChildLayoutAttrs, DeclarativeLayoutKind,
};
pub use loader::{extract_event_handlers, load_layout_from_str, JsonLoader};
pub use properties::is_widget_property;

/// The capability registry used to resolve a JSON widget name to a constructor
/// and to look up a property's declared value kind.
///
/// Built on demand rather than cached in a `static`, matching the call pattern
/// used elsewhere in the crate (`control_backend::custom::mount_widget_of_kind`,
/// `lib::create_widget_of_kind`): the registry is a handful of `Vec`/`HashMap`
/// insertions, which is negligible next to instantiating a widget tree, and a
/// process-wide `static` would need its own lock and lifetime story for no gain.
///
/// Gated on the full widget set: a stripped profile compiles neither the factory
/// nor the constructors it resolves against, so the JSON path there falls back to
/// the loader's own construction table.
#[cfg(full_widgets)]
pub(crate) fn schema_factory() -> crate::widget::WidgetFactory {
    crate::widget::WidgetFactory::new_with_defaults()
}

/// A registry that answers "not registered" in a build without the full widget
/// set, so [`properties::declared_kind`] can keep a single code path.
#[cfg(not(full_widgets))]
#[derive(Debug, Default)]
pub(crate) struct EmptyFactory;

#[cfg(not(full_widgets))]
impl EmptyFactory {
    /// No capability is registered in this profile.
    pub(crate) fn capability_for_kind_instance(
        &self,
        _widget: &dyn crate::widget::Widget,
    ) -> Option<&crate::widget::capability::WidgetCapability> {
        None
    }

    /// No constructor is registered in this profile.
    pub(crate) fn create(
        &self,
        _name: &str,
        _geometry: crate::core::Rect,
        _text: &str,
    ) -> Option<Box<dyn crate::widget::Widget>> {
        None
    }
}

#[cfg(not(full_widgets))]
pub(crate) fn schema_factory() -> EmptyFactory {
    EmptyFactory
}
