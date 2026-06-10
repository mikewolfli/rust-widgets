use std::time::SystemTime;

/// Unique identifier for an undo command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandId(pub u64);

/// Merge policy for consecutive commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergePolicy {
    /// Cannot merge with any other command.
    Never,
    /// Can merge with the previous command of the same type.
    WithPrevious,
}

/// Describes the scope/effect of a command.
#[derive(Debug, Clone)]
pub struct CommandDescription {
    /// Human-readable text (e.g., "Delete text").
    pub text: String,
    /// Timestamp when the command was created.
    pub timestamp: SystemTime,
    /// Static string identifying the command type.
    pub command_type: &'static str,
}
