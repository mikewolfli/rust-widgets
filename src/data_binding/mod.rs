//! Reactive data binding system — Model → View automatic synchronization.
//!
//! Provides `Binding<T>` for single values, `ObservableList<T>` for collections,
//! and `Computed<T>` for derived values, enabling MVVM-style reactive UIs.
//!
//! # Architecture
//!
//! The system follows a "source of truth + observers" pattern:
//!
//! - **[`Binding<T>`]**: A single reactive value. When the value changes via
//!   [`set`](Binding::set), all registered listeners are notified. Supports
//!   two-way binding via [`bind_to`](Binding::bind_to).
//! - **[`ObservableList<T>`]**: An observable collection that fires
//!   notifications on structural mutations (push, pop, insert, remove, clear).
//! - **[`Computed<T>`]**: A derived value computed from a closure. Marked dirty
//!   via [`invalidate`](Computed::invalidate) and lazily recomputed on
//!   [`get`](Computed::get).
//! - **Traits**: [`BindingListener`] and [`BoxedListener`] define the listener
//!   contract. [`FnListener`] wraps a closure as a listener.
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

mod binding;
mod computed;
mod list;
mod macros;
mod traits;

pub use binding::*;
pub use computed::*;
pub use list::*;

pub use traits::*;
