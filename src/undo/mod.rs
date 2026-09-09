//! Undo/Redo framework — generic undo stack supporting commands, grouping,
//! and cross-widget undo/redo operations.

mod command;
mod stack;
mod text_command;
mod types;

pub use command::*;
pub use stack::*;
pub(crate) use text_command::TextSnapshotCommand;
pub use types::*;
