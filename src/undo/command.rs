use super::types::*;

/// Trait for undoable commands.
pub trait UndoCommand {
    /// Unique identifier for this command instance.
    fn id(&self) -> CommandId;

    /// Description of the command (for UI display).
    fn description(&self) -> CommandDescription;

    /// Execute the command (forward action).
    fn execute(&mut self) -> Result<(), String>;

    /// Undo the command (reverse action).
    fn undo(&mut self) -> Result<(), String>;

    /// Redo the command. Default implementation calls `execute`.
    fn redo(&mut self) -> Result<(), String> {
        self.execute()
    }

    /// Returns the merge policy for this command.
    fn merge_policy(&self) -> MergePolicy {
        MergePolicy::Never
    }

    /// Try to merge with a previous command. Returns true if merged.
    fn try_merge(&mut self, _previous: &dyn UndoCommand) -> bool {
        false
    }
}
