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

mod element;
mod events;
mod layout;
mod loader;

pub use element::BoundJsonLayout;
pub use events::{
    clear_global_handlers, invoke_global_handler, register_global_handler, EventHandlerContext,
    EventHandlerMap,
};
pub use layout::{
    add_spacer_to_layout, add_widget_to_layout, create_layout_from_kind, parse_layout_kind,
    store_layout, ChildLayoutAttrs, DeclarativeLayoutKind,
};
pub use loader::{extract_event_handlers, load_layout_from_str, JsonLoader};
