//! Crate-root surface: re-export groups and the procedural API they name.
//!
//! `widgets`, `platform`, `signals` and `compat` are `pub use` surfaces grouped by
//! subject. `host` and `control` hold the crate-root procedural API that used to be
//! defined in `src/lib.rs`; it moved here so the façade stays within the file-size
//! limit, and `src/lib.rs` re-exports the union so every historical
//! `rust_widgets::<name>` path keeps resolving.
mod compat;
mod control;
mod host;
mod platform;
mod signals;
mod widgets;

pub use compat::*;
pub use control::*;
pub use host::*;
pub use platform::*;
pub use signals::*;
pub use widgets::*;
