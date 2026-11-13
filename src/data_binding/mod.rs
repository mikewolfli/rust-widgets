// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reactive data binding system — Model → View automatic synchronization.
//!
//! Provides `Binding<T>` for single values, `ObservableList<T>` for collections,
//! and `Computed<T>` for derived values, enabling MVVM-style reactive UIs.
//!
//! # Architecture
//!
//! The system follows a "source of truth + observers" pattern:
//!
//! - **`Binding<T>`**: A single reactive value. When the value changes via
//!   `set`, all registered listeners are notified. Supports
//!   two-way binding via `bind_to`.
//! - **`ObservableList<T>`**: An observable collection that fires
//!   notifications on structural mutations (push, pop, insert, remove, clear).
//! - **`Computed<T>`**: A derived value computed from a closure. Marked dirty
//!   via `invalidate` and lazily recomputed on
//!   `get`.
//! - **Traits**: `BindingListener` and `BoxedListener` define the listener
//!   contract. `FnListener` wraps a closure as a listener.
//! - **Macros**: [`binding!`](crate::binding!) and [`computed!`](crate::computed!)
//!   provide convenient construction syntax.
//!
//! # Example
//!
//! ```rust
//! use rust_widgets::data_binding::{Binding, FnListener};
//!
//! let mut name = Binding::new("World".to_string());
//! let mut greeting = String::new();
//!
//! name.subscribe("log", Box::new(FnListener::new(|key, _op| {
//!     println!("[{}] name changed!", key);
//! })));
//!
//! name.set("Rust".to_string());
//! assert_eq!(name.get(), "Rust");
//! ```
//!
//! # Reachability
//!
//! **State:** Partly reserved. The declarative [`Binding`]/[`BindingList`]/[`Computed`] model has no
//! production consumer yet, but the **widget bridge is live**: [`bind_widget_property`] is called by
//! `spinbox`, `line_edit` and `slider` to bind a property to a binding, and [`pull_widget_property`]
//! is the read half. The earlier wording here ("no production consumer") was false for that half
//! and is corrected rather than deleted (W6-27). Removal condition for the remaining declarative
//! model: when the JSON path adopts it, or when it is confirmed redundant with the property contract.
//! The widget bridge follows the property contract and is not a removal candidate.

mod binding;
#[cfg(all(full_widgets, not(alloc_frugal)))]
pub mod chart;
mod computed;
mod list;
mod macros;
mod traits;
mod widget_bridge;

pub use binding::*;
#[cfg(all(full_widgets, not(alloc_frugal)))]
pub use chart::*;
pub use computed::*;
pub use list::*;
pub use traits::*;
pub use widget_bridge::*;
